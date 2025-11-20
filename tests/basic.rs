use lfrlock::LfrLock;

#[derive(Debug, Clone)]
struct Data {
    value: i32,
}

#[test]
fn test_lfrlock_read_write() {
    let lock = LfrLock::new(Data { value: 0 });

    // Test write operation (closure style)
    // 测试写入操作（闭包方式）
    for i in 1..=10 {
        lock.write_with(|old_data| Data {
            value: old_data.value + 1,
        });

        // Verify value after each write
        // 在每次写入后验证值
        let data = lock.read();
        assert_eq!(data.value, i);
    }

    // Check final value
    // 检查最终值
    let data = lock.read();
    assert_eq!(data.value, 10);
}

#[test]
fn test_lock_clone() {
    let lock = LfrLock::new(Data { value: 0 });

    // Clone lock
    // 克隆锁
    let lock2 = lock.clone();

    // Both lock instances can write
    // 两个锁实例都可以写入
    lock.write_with(|old_data| Data {
        value: old_data.value + 10,
    });

    lock2.write_with(|old_data| Data {
        value: old_data.value + 5,
    });

    // Verify final value
    // 验证最终值
    let data = lock.read();
    assert_eq!(data.value, 15);
}

#[test]
fn test_multiple_lock_instances() {
    let lock = LfrLock::new(Data { value: 42 });

    // Clone lock
    // 克隆锁
    let lock2 = lock.clone();

    // Both lock instances can read
    // 两个锁实例都可以读取
    let data1 = lock.read();
    let data2 = lock2.read();

    assert_eq!(data1.value, 42);
    assert_eq!(data2.value, 42);

    // Use lock for writing
    // 使用 lock 进行写入
    lock.write_with(|_| Data { value: 100 });

    // Both lock instances should see the new value
    // 两个锁实例应该都能看到新值
    let data1 = lock.read();
    let data2 = lock2.read();

    assert_eq!(data1.value, 100);
    assert_eq!(data2.value, 100);
}
