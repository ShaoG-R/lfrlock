use lfrlock::LfrLock;
use std::thread;

#[derive(Debug, Clone)]
struct Data {
    value: i32,
}

#[test]
fn test_multiple_writers() {
    let lock = LfrLock::new(Data { value: 0 });

    // Create multiple writers
    // 创建多个写者
    let mut handles = vec![];

    for _ in 0..4 {
        let lock_clone = lock.clone();
        let handle = thread::spawn(move || {
            for _ in 0..25 {
                lock_clone.update(|old_data| Data {
                    value: old_data.value + 1,
                });
            }
        });
        handles.push(handle);
    }

    // Wait for all writers to complete
    // 等待所有写者完成
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final value should be 100 (4 threads * 25 increments)
    // Verify final value
    // 验证最终值应该是 100 (4 个线程 * 25 次增量)
    let data = lock.read();
    assert_eq!(data.value, 100);
}

#[test]
fn test_multiple_readers_and_writers() {
    let lock = LfrLock::new(Data { value: 0 });

    let mut handles = vec![];

    // Start 2 writers
    // 启动 2 个写者
    for _ in 0..2 {
        let lock_clone = lock.clone();
        let handle = thread::spawn(move || {
            for _ in 0..50 {
                lock_clone.update(|old_data| Data {
                    value: old_data.value + 1,
                });
            }
        });
        handles.push(handle);
    }

    // Start 3 readers
    // 启动 3 个读者
    for _ in 0..3 {
        let lock_clone = lock.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let data = lock_clone.read();
                // Readers should always see valid state
                // 读者应该总是能看到有效的状态
                assert!(data.value >= 0 && data.value <= 100);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    // 等待所有线程完成
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final value
    // 验证最终值
    let data = lock.read();
    assert_eq!(data.value, 100);
}
