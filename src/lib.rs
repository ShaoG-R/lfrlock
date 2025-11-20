use antidote::{Mutex, MutexGuard};
use smr_swap::{ReaderGuard, SwapReader, Swapper};
use std::fmt;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// LfrLock (Lock-Free Read Lock) - 读取永不阻塞，写入使用 Mutex 串行化
///
/// 类似于 `std::sync::Mutex`，统一的类型同时支持读写操作。
/// 核心特性：读取操作无锁且永不阻塞；写入操作涉及复制旧数据、修改、然后原子替换。
pub struct LfrLock<T> {
    swapper: Arc<Mutex<Swapper<T>>>,
    reader: SwapReader<T>,
}

impl<T: 'static> LfrLock<T> {
    /// 创建新的 LfrLock
    #[inline]
    pub fn new(initial: T) -> Self {
        let (swapper, reader) = smr_swap::new_smr_pair(initial);

        LfrLock {
            swapper: Arc::new(Mutex::new(swapper)),
            reader,
        }
    }

    /// 更新数据 - 直接替换
    #[inline]
    pub fn update(&self, new_t: T) {
        let mut swapper = self.swapper.lock();
        swapper.update(new_t);
    }

    /// 写入操作（闭包方式）
    #[inline]
    pub fn write_with<F>(&self, f: F)
    where
        F: FnOnce(&T) -> T,
    {
        // 获取 Mutex 锁，确保同一时间只有一个写者在写入
        let mut swapper = self.swapper.lock();

        // 1. 读取旧数据并执行更新逻辑
        // 使用 reader 读取当前值
        let guard = self.reader.load();
        let new_t = f(&*guard);
        drop(guard); // 显式释放读锁

        // 2. 换入新的 "T" 状态
        swapper.update(new_t);
    }

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

    /// 尝试获取写入锁
    #[inline]
    pub fn try_write(&self) -> Option<WriteGuard<'_, T>>
    where
        T: Clone,
    {
        let swapper_guard = self.swapper.try_lock().ok()?;

        let guard = self.reader.load();
        let data = guard.clone();

        Some(WriteGuard {
            swapper_guard,
            data: ManuallyDrop::new(data),
        })
    }

    /// 读取数据 - 永不阻塞
    #[inline]
    pub fn read(&self) -> ReaderGuard<'_, T> {
        self.reader.load()
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
            reader: self.reader.fork(),
        }
    }
}

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

        let guard = lock.reader.load();
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
        // 从 ManuallyDrop 中取出数据
        let new_data = unsafe { ManuallyDrop::take(&mut self.data) };

        // 执行状态切换
        self.swapper_guard.update(new_data);
    }
}

// 安全性：T 必须是 Send
// LfrLock 是 Send (如果 T 是 Send)，因为 SwapReader 是 Send，Arc<Mutex<Swapper>> 是 Send。
unsafe impl<T: Send + 'static> Send for LfrLock<T> {}

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
            lock.write_with(|old_data| Data {
                value: old_data.value + 1,
            });

            // 在每次写入后验证值
            let data = lock.read();
            assert_eq!(data.value, i);
        }

        // 检查最终值
        let data = lock.read();
        assert_eq!(data.value, 10);
    }

    #[test]
    fn test_write_guard() {
        let lock = LfrLock::new(Data { value: 0 });

        // 使用 WriteGuard 进行写入
        for i in 1..=10 {
            {
                let mut write_guard = lock.write();
                write_guard.value += 1; // 直接修改，无需闭包
            } // guard drop，自动提交

            // 在每次写入后验证值
            let data = lock.read();
            assert_eq!(data.value, i);
        }

        // 检查最终值
        let data = lock.read();
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
                    lock_clone.write_with(|old_data| Data {
                        value: old_data.value + 1,
                    });
                }
            });
            handles.push(handle);
        }

        // 等待所有写者完成
        for handle in handles {
            handle.join().unwrap();
        }

        // 验证最终值应该是 100 (4 个线程 * 25 次增量)
        let data = lock.read();
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
                    lock_clone.write_with(|old_data| Data {
                        value: old_data.value + 1,
                    });
                }
            });
            handles.push(handle);
        }

        // 启动 3 个读者
        for _ in 0..3 {
            let lock_clone = lock.clone();
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    let data = lock_clone.read();
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
        let data = lock.read();
        assert_eq!(data.value, 100);
    }

    #[test]
    fn test_lock_clone() {
        let lock = LfrLock::new(Data { value: 0 });

        // 克隆锁
        let lock2 = lock.clone();

        // 两个锁实例都可以写入
        lock.write_with(|old_data| Data {
            value: old_data.value + 10,
        });

        lock2.write_with(|old_data| Data {
            value: old_data.value + 5,
        });

        // 验证最终值
        let data = lock.read();
        assert_eq!(data.value, 15);
    }

    #[test]
    fn test_multiple_lock_instances() {
        let lock = LfrLock::new(Data { value: 42 });

        // 克隆锁
        let lock2 = lock.clone();

        // 两个锁实例都可以读取
        let data1 = lock.read();
        let data2 = lock2.read();

        assert_eq!(data1.value, 42);
        assert_eq!(data2.value, 42);

        // 使用 lock 进行写入
        lock.write_with(|_| Data { value: 100 });

        // 两个锁实例应该都能看到新值
        let data1 = lock.read();
        let data2 = lock2.read();

        assert_eq!(data1.value, 100);
        assert_eq!(data2.value, 100);
    }
}
