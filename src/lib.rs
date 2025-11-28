use antidote::{Mutex, MutexGuard};
use smr_swap::{LocalReader, ReadGuard, SmrSwap};
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
pub struct LfrLock<T: 'static> {
    swap: Arc<Mutex<SmrSwap<T>>>,
    local: LocalReader<T>,
}

impl<T: 'static> LfrLock<T> {
    /// Create a new LfrLock
    ///
    /// 创建新的 LfrLock
    #[inline]
    pub fn new(initial: T) -> Self {
        let swap = SmrSwap::new(initial);
        let local = swap.local();

        LfrLock {
            swap: Arc::new(Mutex::new(swap)),
            local,
        }
    }

    /// Store a new value, making it visible to readers.
    ///
    /// The old value is retired and will be garbage collected when safe.
    ///
    /// 存储新值，使其对读者可见。
    ///
    /// 旧值已退休，将在安全时被垃圾回收。
    #[inline]
    pub fn store(&self, new_value: T) {
        let mut swap = self.swap.lock();
        swap.store(new_value);
    }

    /// Atomically swap the current value with a new one.
    ///
    /// Returns the old value.
    ///
    /// 原子地将当前值与新值交换。
    ///
    /// 返回旧的值。
    #[inline]
    pub fn swap(&self, new_value: T) -> T
    where
        T: Clone,
    {
        self.swap.lock().swap(new_value)
    }

    /// Update the value using a closure.
    ///
    /// The closure receives the current value and should return the new value.
    ///
    /// 使用闭包更新值。
    ///
    /// 闭包接收当前值并应返回新值。
    #[inline]
    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&T) -> T,
    {
        self.swap.lock().update(f);
    }

    /// Apply a closure function to the current value and return a guard to the new value.
    ///
    /// The closure receives a reference to the current value and returns a new value.
    ///
    /// 对当前值应用闭包函数并返回新值的守卫。
    ///
    /// 闭包接收当前值的引用，返回新值。
    #[inline]
    pub fn update_and_fetch<F>(&self, f: F) -> ReadGuard<'_, T>
    where
        F: FnOnce(&T) -> T,
    {
        self.swap.lock().update(f);
        self.local.load()
    }

    /// Apply a closure function to the current value and return a guard to the old value.
    ///
    /// The closure receives the current value and should return the new value.
    /// Returns a guard to the old value (before update).
    ///
    /// 对当前值应用闭包函数并返回旧值的守卫。
    ///
    /// 闭包接收当前值并应返回新值。
    /// 返回旧值（更新前）的守卫。
    #[inline]
    pub fn fetch_and_update<F>(&self, f: F) -> ReadGuard<'_, T>
    where
        F: FnOnce(&T) -> T,
    {
        let old_guard = self.local.load();
        self.swap.lock().update(f);
        old_guard
    }

    /// Apply a closure function to the current value and transform the result.
    ///
    /// This method reads the current value, applies the closure to transform it,
    /// and returns the transformed result.
    ///
    /// 对当前值应用闭包函数并转换结果。
    ///
    /// 这个方法读取当前值，应用闭包进行转换，并返回转换后的结果。
    #[inline]
    pub fn map<F, U>(&self, f: F) -> U
    where
        F: FnOnce(&T) -> U,
    {
        let guard = self.local.load();
        f(&*guard)
    }

    /// Apply a closure function to the current value, returning Some if the closure returns true.
    ///
    /// 对当前值应用闭包函数，如果闭包返回 true 则返回 Some。
    #[inline]
    pub fn filter<F>(&self, f: F) -> Option<ReadGuard<'_, T>>
    where
        F: FnOnce(&T) -> bool,
    {
        let guard = self.local.load();
        if f(&*guard) {
            Some(guard)
        } else {
            None
        }
    }

    /// Get the current value by cloning.
    ///
    /// 通过克隆获取当前值。
    #[inline]
    pub fn get(&self) -> T
    where
        T: Clone,
    {
        (&*self.local.load()).clone()
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
        let swap_guard = self.swap.try_lock().ok()?;

        let guard = self.local.load();
        let data = (*guard).clone();

        Some(WriteGuard {
            swap_guard,
            data: ManuallyDrop::new(data),
        })
    }

    /// Read data - never blocks
    ///
    /// 读取数据 - 永不阻塞
    #[inline]
    pub fn read(&self) -> ReadGuard<'_, T> {
        self.local.load()
    }
}

impl<T: Default + 'static> Default for LfrLock<T> {
    /// Create a new LfrLock with the default value.
    ///
    /// 使用默认值创建一个新的 LfrLock。
    #[inline]
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T: 'static> From<T> for LfrLock<T> {
    /// Create a new LfrLock from a value.
    ///
    /// 从一个值创建一个新的 LfrLock。
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
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
            swap: self.swap.clone(),
            local: self.local.clone(),
        }
    }
}

/// Write Guard - Provides direct mutable access, automatically commits changes on Drop
/// Holds Mutex lock to ensure exclusive write access
///
/// 写入保护器 - 提供直接的可变访问，在 Drop 时自动提交更改
/// 持有 Mutex 锁，确保独占写入访问
pub struct WriteGuard<'a, T: 'static> {
    swap_guard: MutexGuard<'a, SmrSwap<T>>,
    data: ManuallyDrop<T>,
}

impl<'a, T: 'static + Clone> WriteGuard<'a, T> {
    #[inline]
    fn new(lock: &'a LfrLock<T>) -> Self {
        // 获取 Mutex 锁
        let swap_guard = lock.swap.lock();

        let guard = lock.local.load();
        let data = (*guard).clone();

        WriteGuard {
            swap_guard,
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
        self.swap_guard.store(new_data);
    }
}

