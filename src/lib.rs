#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

use core::fmt;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use smr_swap::{LocalReader, ReadGuard, SmrReader, SmrSwap};

#[cfg(feature = "std")]
use std::sync::Arc;

#[cfg(not(feature = "std"))]
use alloc::sync::Arc;

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
        if f(&*guard) { Some(guard) } else { None }
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

    /// Create a factory for creating new `LfrLock` instances.
    ///
    /// The returned factory is `Sync` + `Clone` and can be shared across threads.
    ///
    /// 创建用于创建新 `LfrLock` 实例的工厂。
    ///
    /// 返回的工厂是 `Sync` + `Clone` 的，可以在线程之间共享。
    #[inline]
    pub fn factory(&self) -> LfrLockFactory<T> {
        LfrLockFactory {
            swap: self.swap.clone(),
            reader: self.local.share(),
        }
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

/// Factory for creating `LfrLock` instances.
///
/// This factory is `Sync` + `Clone` and can be shared across threads.
/// It allows creating new `LfrLock` instances for the current thread.
///
/// 用于创建 `LfrLock` 实例的工厂。
///
/// 该工厂是 `Sync` + `Clone` 的，可以在线程之间共享。
/// 它允许为当前线程创建新的 `LfrLock` 实例。
pub struct LfrLockFactory<T: 'static> {
    swap: Arc<Mutex<SmrSwap<T>>>,
    reader: SmrReader<T>,
}

impl<T: 'static> LfrLockFactory<T> {
    /// Create a new factory with the initial value.
    ///
    /// 使用初始值创建一个新工厂。
    #[inline]
    pub fn new(initial: T) -> Self {
        let swap = SmrSwap::new(initial);
        let reader = swap.reader();
        Self {
            swap: Arc::new(Mutex::new(swap)),
            reader,
        }
    }

    /// Create a new lock instance for the current thread.
    ///
    /// 为当前线程创建一个新的锁实例。
    #[inline]
    pub fn create(&self) -> LfrLock<T> {
        LfrLock {
            swap: self.swap.clone(),
            local: self.reader.local(),
        }
    }
}

impl<T: 'static> Clone for LfrLockFactory<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            swap: self.swap.clone(),
            reader: self.reader.clone(),
        }
    }
}

#[cfg(feature = "std")]
mod lock_impl {
    use std::ops::{Deref, DerefMut};

    /// Like `std::sync::Mutex` except that it does not poison itself.
    pub struct Mutex<T: ?Sized>(std::sync::Mutex<T>);

    impl<T> Mutex<T> {
        /// Like `std::sync::Mutex::new`.
        #[inline]
        pub fn new(t: T) -> Mutex<T> {
            Mutex(std::sync::Mutex::new(t))
        }
    }

    impl<T: ?Sized> Mutex<T> {
        /// Like `std::sync::Mutex::lock`.
        #[inline]
        pub fn lock<'a>(&'a self) -> MutexGuard<'a, T> {
            MutexGuard(self.0.lock().unwrap_or_else(|e| e.into_inner()))
        }

        /// Like `std::sync::Mutex::try_lock`.
        #[inline]
        pub fn try_lock<'a>(&'a self) -> TryLockResult<MutexGuard<'a, T>> {
            match self.0.try_lock() {
                Ok(t) => Ok(MutexGuard(t)),
                Err(std::sync::TryLockError::Poisoned(e)) => Ok(MutexGuard(e.into_inner())),
                Err(std::sync::TryLockError::WouldBlock) => Err(TryLockError(())),
            }
        }
    }

    /// Like `std::sync::MutexGuard`.
    #[must_use]
    pub struct MutexGuard<'a, T: ?Sized + 'a>(std::sync::MutexGuard<'a, T>);

    impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
        type Target = T;

        #[inline]
        fn deref(&self) -> &T {
            self.0.deref()
        }
    }

    impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
        #[inline]
        fn deref_mut(&mut self) -> &mut T {
            self.0.deref_mut()
        }
    }

    /// Like `std::sync::TryLockResult`.
    pub type TryLockResult<T> = Result<T, TryLockError>;

    /// Like `std::sync::TryLockError`.
    #[derive(Debug)]
    pub struct TryLockError(());
}

#[cfg(not(feature = "std"))]
mod lock_impl {
    use core::ops::{Deref, DerefMut};

    /// `spin::Mutex` wrapper to match `std::sync::Mutex` API.
    pub struct Mutex<T: ?Sized>(spin::Mutex<T>);

    impl<T> Mutex<T> {
        #[inline]
        pub fn new(t: T) -> Mutex<T> {
            Mutex(spin::Mutex::new(t))
        }
    }

    impl<T: ?Sized> Mutex<T> {
        #[inline]
        pub fn lock(&self) -> MutexGuard<'_, T> {
            MutexGuard(self.0.lock())
        }

        #[inline]
        pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
            match self.0.try_lock() {
                Some(guard) => Ok(MutexGuard(guard)),
                None => Err(TryLockError(())),
            }
        }
    }

    #[must_use]
    pub struct MutexGuard<'a, T: ?Sized + 'a>(spin::MutexGuard<'a, T>);

    impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
        type Target = T;

        #[inline]
        fn deref(&self) -> &T {
            &*self.0
        }
    }

    impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
        #[inline]
        fn deref_mut(&mut self) -> &mut T {
            &mut *self.0
        }
    }

    pub type TryLockResult<T> = Result<T, TryLockError>;

    #[derive(Debug)]
    pub struct TryLockError(());
}

use lock_impl::*;
