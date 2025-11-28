use lfrlock::LfrLock;

#[derive(Debug, Clone, PartialEq)]
struct Data {
    value: i32,
}

#[test]
fn test_lfrlock_read_write() {
    let lock = LfrLock::new(Data { value: 0 });

    // Test update operation (closure style)
    // 测试 update 操作（闭包方式）
    for i in 1..=10 {
        lock.update(|old_data| Data {
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
fn test_store() {
    let lock = LfrLock::new(Data { value: 0 });

    // Test store - direct replacement
    // 测试 store - 直接替换
    lock.store(Data { value: 42 });
    assert_eq!(lock.read().value, 42);

    lock.store(Data { value: 100 });
    assert_eq!(lock.read().value, 100);
}

#[test]
fn test_swap() {
    let lock = LfrLock::new(Data { value: 10 });

    // Test swap - returns old value
    // 测试 swap - 返回旧值
    let old = lock.swap(Data { value: 20 });
    assert_eq!(old.value, 10);
    assert_eq!(lock.read().value, 20);

    let old = lock.swap(Data { value: 30 });
    assert_eq!(old.value, 20);
    assert_eq!(lock.read().value, 30);
}

#[test]
fn test_get() {
    let lock = LfrLock::new(Data { value: 42 });

    // Test get - returns cloned value
    // 测试 get - 返回克隆的值
    let value = lock.get();
    assert_eq!(value.value, 42);

    lock.store(Data { value: 100 });
    let value = lock.get();
    assert_eq!(value.value, 100);
}

#[test]
fn test_map() {
    let lock = LfrLock::new(Data { value: 42 });

    // Test map - transform and return result
    // 测试 map - 转换并返回结果
    let doubled = lock.map(|data| data.value * 2);
    assert_eq!(doubled, 84);

    let as_string = lock.map(|data| format!("value: {}", data.value));
    assert_eq!(as_string, "value: 42");
}

#[test]
fn test_filter() {
    let lock = LfrLock::new(Data { value: 42 });

    // Test filter - conditional read
    // 测试 filter - 条件读取
    let result = lock.filter(|data| data.value > 40);
    assert!(result.is_some());
    assert_eq!(result.unwrap().value, 42);

    let result = lock.filter(|data| data.value > 50);
    assert!(result.is_none());
}

#[test]
fn test_lock_clone() {
    let lock = LfrLock::new(Data { value: 0 });

    // Clone lock
    // 克隆锁
    let lock2 = lock.clone();

    // Both lock instances can write
    // 两个锁实例都可以写入
    lock.update(|old_data| Data {
        value: old_data.value + 10,
    });

    lock2.update(|old_data| Data {
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

    // Use store for direct replacement
    // 使用 store 进行直接替换
    lock.store(Data { value: 100 });

    // Both lock instances should see the new value
    // 两个锁实例应该都能看到新值
    let data1 = lock.read();
    let data2 = lock2.read();

    assert_eq!(data1.value, 100);
    assert_eq!(data2.value, 100);
}

#[test]
fn test_from_trait() {
    // Test From trait
    // 测试 From trait
    let lock: LfrLock<Data> = Data { value: 42 }.into();
    assert_eq!(lock.read().value, 42);

    let lock = LfrLock::from(Data { value: 100 });
    assert_eq!(lock.read().value, 100);
}

#[test]
fn test_default_trait() {
    // Test Default trait
    // 测试 Default trait
    let lock: LfrLock<i32> = LfrLock::default();
    assert_eq!(*lock.read(), 0);

    let lock: LfrLock<String> = LfrLock::default();
    assert_eq!(*lock.read(), "");
}
