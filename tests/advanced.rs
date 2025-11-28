use lfrlock::LfrLock;

#[derive(Debug, Clone, PartialEq)]
struct Data {
    value: i32,
}

#[test]
fn test_write_guard() {
    let lock = LfrLock::new(Data { value: 0 });

    // Use WriteGuard for writing
    // 使用 WriteGuard 进行写入
    for i in 1..=10 {
        {
            let mut write_guard = lock.write();
            write_guard.value += 1; // Modify directly, no closure needed / 直接修改，无需闭包
        } // guard drop, auto commit / guard drop，自动提交

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
fn test_try_write() {
    let lock = LfrLock::new(Data { value: 0 });

    // Test try_write - should succeed when no other writer
    // 测试 try_write - 没有其他写者时应该成功
    {
        let guard = lock.try_write();
        assert!(guard.is_some());
        let mut guard = guard.unwrap();
        guard.value = 42;
    }
    assert_eq!(lock.read().value, 42);
}

#[test]
fn test_update_and_fetch() {
    let lock = LfrLock::new(Data { value: 0 });

    // Test update_and_fetch - returns guard to new value
    // 测试 update_and_fetch - 返回新值的守卫
    let new_guard = lock.update_and_fetch(|old| Data {
        value: old.value + 10,
    });
    assert_eq!(new_guard.value, 10);

    let new_guard = lock.update_and_fetch(|old| Data {
        value: old.value * 2,
    });
    assert_eq!(new_guard.value, 20);
}

#[test]
fn test_fetch_and_update() {
    let lock = LfrLock::new(Data { value: 100 });

    // Test fetch_and_update - returns guard to old value
    // 测试 fetch_and_update - 返回旧值的守卫
    let old_guard = lock.fetch_and_update(|old| Data {
        value: old.value + 50,
    });
    assert_eq!(old_guard.value, 100); // old value
    assert_eq!(lock.read().value, 150); // new value

    let old_guard = lock.fetch_and_update(|old| Data {
        value: old.value * 2,
    });
    assert_eq!(old_guard.value, 150); // old value
    assert_eq!(lock.read().value, 300); // new value
}

#[test]
fn test_chained_operations() {
    let lock = LfrLock::new(Data { value: 1 });

    // Chain multiple operations
    // 链式调用多个操作
    lock.update(|d| Data { value: d.value + 1 }); // 2
    lock.update(|d| Data { value: d.value * 3 }); // 6
    lock.update(|d| Data { value: d.value - 1 }); // 5

    assert_eq!(lock.get().value, 5);

    // Use map to compute without modifying
    // 使用 map 计算而不修改
    let squared = lock.map(|d| d.value * d.value);
    assert_eq!(squared, 25);
    assert_eq!(lock.get().value, 5); // unchanged
}
