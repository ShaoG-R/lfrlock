use swmr_epoch::{EpochGcDomain, EpochPtr, GcHandle, LocalEpoch, PinGuard};
use antidote::{Mutex, MutexGuard};
use std::sync::Arc;
use std::ops::{Deref, DerefMut};
use std::mem::ManuallyDrop;
use std::fmt;

/// LfrLock (Lock-Free Read Lock) - 读取永不阻塞，写入使用 Mutex 串行化
/// 
/// 类似于 `std::sync::Mutex`，统一的类型同时支持读写操作。
/// 核心特性：读取操作无锁且永不阻塞；写入操作涉及复制旧数据、修改、然后原子替换。
pub struct LfrLock<T> {
    inner: Arc<LfrLockInner<T>>,
}

struct LfrLockInner<T> {
    data: EpochPtr<T>,
    gc: Mutex<GcHandle>,
    domain: EpochGcDomain,
}

impl<T: 'static> LfrLock<T> {
    /// 创建新的 LfrLock
    #[inline]
    pub fn new(initial: T) -> Self {
        let (gc, domain) = EpochGcDomain::builder()
            .auto_reclaim_threshold(None)
            .cleanup_interval(2)
            .build();
        
        let data = EpochPtr::new(initial);
        
        LfrLock {
            inner: Arc::new(LfrLockInner {
                data,
                gc: Mutex::new(gc),
                domain,
            }),
        }
    }

    /// 更新数据 - 直接替换
    #[inline]
    pub fn update(&self, new_t: T) {
        let mut gc = self.inner.gc.lock();
        self.inner.data.store(new_t, &mut *gc);
        gc.collect();
    }

    /// 写入操作（闭包方式）
    #[inline]
    pub fn write_with<F>(&self, mut updater: F, guard: &PinGuard)
    where
        F: FnMut(&T) -> T,
    {
        // 获取 Mutex 锁，确保同一时间只有一个写者在写入
        let mut gc = self.inner.gc.lock();
        
        // 1. 读取旧数据并执行更新逻辑
        let old_t = self.inner.data.load(guard);
        let new_t = updater(old_t);

        // 2. 换入新的 "T" 状态
        self.inner.data.store(new_t, &mut *gc);

        // 3. 执行垃圾回收
        gc.collect();
    }

    /// 写入操作（Guard 方式）- 需要 T 实现 Clone
    /// 
    /// 返回 WriteGuard，允许直接修改数据，在 drop 时自动提交。
    /// 获取 Mutex 锁，确保串行化写入。
    #[inline]
    pub fn write<'g>(&'g self, guard: &'g PinGuard) -> WriteGuard<'g, T>
    where
        T: Clone,
    {
        WriteGuard::new(self, guard)
    }

    /// 尝试获取写入锁
    #[inline]
    pub fn try_write<'g>(&'g self, guard: &'g PinGuard) -> Option<WriteGuard<'g, T>>
    where
        T: Clone,
    {
        let gc_guard = self.inner.gc.try_lock().ok()?;
        
        let old_t = self.inner.data.load(guard);
        let data = old_t.clone();
        
        Some(WriteGuard {
            lock: self,
            gc_guard,
            data: ManuallyDrop::new(data),
        })
    }

    /// 注册读者，获取 LocalEpoch
    #[inline]
    pub fn register(&self) -> LocalEpoch {
        self.inner.domain.register_reader()
    }

    /// 读取数据 - 永不阻塞
    ///
    /// PinGuard 必须传入，以确保内存安全。
    #[inline]
    pub fn read<'g>(&self, guard: &'g PinGuard) -> &'g T {
        // 使用 EpochPtr 加载当前状态
        // PinGuard 确保内存安全
        self.inner.data.load(guard)
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
        let local_epoch = self.register();
        let guard = local_epoch.pin();
        let data = self.read(&guard);
        f.debug_struct("LfrLock")
            .field("data", data)
            .finish()
    }
}

impl<T> Clone for LfrLock<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// 写入保护器 - 提供直接的可变访问，在 Drop 时自动提交更改
/// 持有 Mutex 锁，确保独占写入访问
pub struct WriteGuard<'a, T: 'static> {
    lock: &'a LfrLock<T>,
    gc_guard: MutexGuard<'a, GcHandle>,
    data: ManuallyDrop<T>,
}

impl<'a, T: 'static + Clone> WriteGuard<'a, T> {
    #[inline]
    fn new(lock: &'a LfrLock<T>, guard: &'a PinGuard) -> Self {
        // 获取 Mutex 锁
        let gc_guard = lock.inner.gc.lock();

        let old_t = lock.inner.data.load(guard);
        let data = old_t.clone();
        
        WriteGuard {
            lock,
            gc_guard,
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
        // 从 ManuallyDrop 中取出数据
        // 安全性：self.data 在这里被消费，之后 WriteGuard 销毁时不会再次 drop data
        let new_data = unsafe { ManuallyDrop::take(&mut self.data) };
        
        // 执行状态切换
        self.lock.inner.data.store(new_data, &mut *self.gc_guard);
        self.gc_guard.collect();
    }
}

// 安全性：T 必须是 Send/Sync
unsafe impl<T: Send> Send for LfrLock<T> {}
unsafe impl<T: Send + Sync> Sync for LfrLock<T> {}

// ========== 使用示例和测试 ==========

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[derive(Debug, Clone)]
    struct Data {
        value: i32,
    }

    #[test]
    fn test_lfrlock_read_write() {
        let lock = LfrLock::new(Data { value: 0 });

        // 测试写入操作（闭包方式）
        for i in 1..=10 {
            let local_epoch = lock.register();
            let guard = local_epoch.pin();
            lock.write_with(|old_data| Data {
                value: old_data.value + 1,
            }, &guard);

            // 在每次写入后验证值
            let local_epoch = lock.register();
            let guard = local_epoch.pin();
            let data = lock.read(&guard);
            assert_eq!(data.value, i);
        }

        // 检查最终值
        let local_epoch = lock.register();
        let guard = local_epoch.pin();
        let data = lock.read(&guard);
        assert_eq!(data.value, 10);
    }

    #[test]
    fn test_write_guard() {
        let lock = LfrLock::new(Data { value: 0 });

        // 使用 WriteGuard 进行写入
        for i in 1..=10 {
            {
                let local_epoch = lock.register();
                let guard = local_epoch.pin();
                let mut write_guard = lock.write(&guard);
                write_guard.value += 1;  // 直接修改，无需闭包
            }  // guard drop，自动提交

            // 在每次写入后验证值
            let local_epoch = lock.register();
            let pin_guard = local_epoch.pin();
            let data = lock.read(&pin_guard);
            assert_eq!(data.value, i);
        }

        // 检查最终值
        let local_epoch = lock.register();
        let pin_guard = local_epoch.pin();
        let data = lock.read(&pin_guard);
        assert_eq!(data.value, 10);
    }

    #[test]
    fn test_multiple_writers() {
        let lock = LfrLock::new(Data { value: 0 });
        
        // 创建多个写者
        let mut handles = vec![];
        
        for _ in 0..4 {
            let lock_clone = lock.clone();
            let handle = thread::spawn(move || {
                for _ in 0..25 {
                    let local_epoch = lock_clone.register();
                    let guard = local_epoch.pin();
                    lock_clone.write_with(|old_data| Data {
                        value: old_data.value + 1,
                    }, &guard);
                }
            });
            handles.push(handle);
        }
        
        // 等待所有写者完成
        for handle in handles {
            handle.join().unwrap();
        }
        
        // 验证最终值应该是 100 (4 个线程 * 25 次增量)
        let local_epoch = lock.register();
        let guard = local_epoch.pin();
        let data = lock.read(&guard);
        assert_eq!(data.value, 100);
    }

    #[test]
    fn test_multiple_readers_and_writers() {
        let lock = LfrLock::new(Data { value: 0 });
        
        let mut handles = vec![];
        
        // 启动 2 个写者
        for _ in 0..2 {
            let lock_clone = lock.clone();
            let handle = thread::spawn(move || {
                for _ in 0..50 {
                    let local_epoch = lock_clone.register();
                    let guard = local_epoch.pin();
                    lock_clone.write_with(|old_data| Data {
                        value: old_data.value + 1,
                    }, &guard);
                }
            });
            handles.push(handle);
        }
        
        // 启动 3 个读者
        for _ in 0..3 {
            let lock_clone = lock.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let local_epoch = lock_clone.register();
                    let guard = local_epoch.pin();
                    let data = lock_clone.read(&guard);
                    // 读者应该总是能看到有效的状态
                    assert!(data.value >= 0 && data.value <= 100);
                }
            });
            handles.push(handle);
        }
        
        // 等待所有线程完成
        for handle in handles {
            handle.join().unwrap();
        }
        
        // 验证最终值
        let local_epoch = lock.register();
        let guard = local_epoch.pin();
        let data = lock.read(&guard);
        assert_eq!(data.value, 100);
    }

    #[test]
    fn test_lock_clone() {
        let lock = LfrLock::new(Data { value: 0 });
        
        // 克隆锁
        let lock2 = lock.clone();
        
        // 两个锁实例都可以写入
        let local_epoch = lock.register();
        let guard = local_epoch.pin();
        lock.write_with(|old_data| Data {
            value: old_data.value + 10,
        }, &guard);
        
        let local_epoch2 = lock2.register();
        let guard2 = local_epoch2.pin();
        lock2.write_with(|old_data| Data {
            value: old_data.value + 5,
        }, &guard2);
        
        // 验证最终值
        let local_epoch = lock.register();
        let guard = local_epoch.pin();
        let data = lock.read(&guard);
        assert_eq!(data.value, 15);
    }

    #[test]
    fn test_multiple_lock_instances() { 
        let lock = LfrLock::new(Data { value: 42 });
        
        // 克隆锁
        let lock2 = lock.clone();
        
        // 两个锁实例都可以读取
        let local_epoch1 = lock.register();
        let guard1 = local_epoch1.pin();
        let data1 = lock.read(&guard1);
        
        let local_epoch2 = lock2.register();
        let guard2 = local_epoch2.pin();
        let data2 = lock2.read(&guard2);
        
        assert_eq!(data1.value, 42);
        assert_eq!(data2.value, 42);
        
        // 使用 lock 进行写入
        let local_epoch = lock.register();
        let guard = local_epoch.pin();
        lock.write_with(|_| Data { value: 100 }, &guard);
        
        // 两个锁实例应该都能看到新值
        let local_epoch1 = lock.register();
        let guard1 = local_epoch1.pin();
        let data1 = lock.read(&guard1);
        
        let local_epoch2 = lock2.register();
        let guard2 = local_epoch2.pin();
        let data2 = lock2.read(&guard2);
        
        assert_eq!(data1.value, 100);
        assert_eq!(data2.value, 100);
    }
}
