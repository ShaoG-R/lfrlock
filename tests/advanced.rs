use lfrlock::LfrLock;

#[derive(Debug, Clone)]
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
