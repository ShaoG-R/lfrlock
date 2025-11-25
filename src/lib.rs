use antidote::{Mutex, MutexGuard};
use smr_swap::{ReaderGuard, ReaderHandle, Swapper};
use std::fmt;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// LfrLock (Lock-Free Read Lock) - Reads never block, writes are serialized using Mutex
///
/// Similar to `std::sync::Mutex`, a unified type supports both read and write operations.
/// Core features: Read operations are lock-free and never block; write operations involve copying old data, modifying, and then atomically replacing.
///
/// LfrLock (Lock-Free Read Lock) - 读取永不阻塞，写入使用 Mutex 串行化
///
/// 类似于 `std::sync::Mutex`，统一的类型同时支持读写操作。
/// 核心特性：读取操作无锁且永不阻塞；写入操作涉及复制旧数据、修改、然后原子替换。
pub struct LfrLock<T> {
    swapper: Arc<Mutex<Swapper<T>>>,
    handle: ReaderHandle<T>,
}

impl<T: 'static> LfrLock<T> {
    /// Create a new LfrLock
    ///
    /// 创建新的 LfrLock
    #[inline]
    pub fn new(initial: T) -> Self {
        let (swapper, reader) = smr_swap::new_smr_pair(initial);

        LfrLock {
            swapper: Arc::new(Mutex::new(swapper)),
            handle: reader.handle(),
        }
    }

    /// Update data - direct replacement
    ///
    /// 更新数据 - 直接替换
    #[inline]
    pub fn update(&self, new_t: T) {
        let mut swapper = self.swapper.lock();
        swapper.update(new_t);
    }

    /// Write operation (closure style)
    ///
    /// 写入操作（闭包方式）
    #[inline]
    pub fn write_with<F>(&self, f: F)
    where
        F: FnOnce(&T) -> T,
    {
        // Acquire Mutex lock to ensure only one writer writes at a time
        // 获取 Mutex 锁，确保同一时间只有一个写者在写入
        let mut swapper = self.swapper.lock();

        // 1. Read old data and execute update logic
        // 1. 读取旧数据并执行更新逻辑
        // Use handle to read current value
        // 使用 handle 读取当前值
        let guard = self.handle.load();
        let new_t = f(&*guard);
        // Explicitly release read lock
        // 显式释放读锁
        drop(guard);

        // 2. Swap in the new "T" state
        // 2. 换入新的 "T" 状态
        swapper.update(new_t);
    }

    /// Write operation (Guard style) - Requires T to implement Clone
    ///
    /// Returns WriteGuard, allowing direct data modification, automatically committed on drop.
    /// Acquires Mutex lock to ensure serialized writes.
    ///
    /// 写入操作（Guard 方式）- 需要 T 实现 Clone
    ///
    /// 返回 WriteGuard，允许直接修改数据，在 drop 时自动提交。
    /// 获取 Mutex 锁，确保串行化写入。
    #[inline]
    pub fn write(&self) -> WriteGuard<'_, T>
    where
        T: Clone,
    {
        WriteGuard::new(self)
    }

    /// Try to acquire write lock
    ///
    /// 尝试获取写入锁
    #[inline]
    pub fn try_write(&self) -> Option<WriteGuard<'_, T>>
    where
        T: Clone,
    {
        let swapper_guard = self.swapper.try_lock().ok()?;

        let guard = self.handle.load();
        let data = guard.clone();

        Some(WriteGuard {
            swapper_guard,
            data: ManuallyDrop::new(data),
        })
    }

    /// Read data - never blocks
    ///
    /// 读取数据 - 永不阻塞
    #[inline]
    pub fn read(&self) -> ReaderGuard<'_, T> {
        self.handle.load()
    }
}

impl<T: Default + 'static> Default for LfrLock<T> {
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: fmt::Debug + 'static> fmt::Debug for LfrLock<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data = self.read();
        f.debug_struct("LfrLock").field("data", &*data).finish()
    }
}

impl<T: 'static> Clone for LfrLock<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            swapper: self.swapper.clone(),
            handle: self.handle.clone(),
        }
    }
}

/// Write Guard - Provides direct mutable access, automatically commits changes on Drop
/// Holds Mutex lock to ensure exclusive write access
///
/// 写入保护器 - 提供直接的可变访问，在 Drop 时自动提交更改
/// 持有 Mutex 锁，确保独占写入访问
pub struct WriteGuard<'a, T: 'static> {
    swapper_guard: MutexGuard<'a, Swapper<T>>,
    data: ManuallyDrop<T>,
}

impl<'a, T: 'static + Clone> WriteGuard<'a, T> {
    #[inline]
    fn new(lock: &'a LfrLock<T>) -> Self {
        // 获取 Mutex 锁
        let swapper_guard = lock.swapper.lock();

        let guard = lock.handle.load();
        let data = guard.clone();

        WriteGuard {
            swapper_guard,
            data: ManuallyDrop::new(data),
        }
    }
}

impl<'a, T: 'static> Deref for WriteGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T: 'static> DerefMut for WriteGuard<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<'a, T: 'static> Drop for WriteGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        // Take data from ManuallyDrop
        // 从 ManuallyDrop 中取出数据
        let new_data = unsafe { ManuallyDrop::take(&mut self.data) };

        // Execute state swap
        // 执行状态切换
        self.swapper_guard.update(new_data);
    }
}

