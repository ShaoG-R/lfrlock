# LfrLock: Lock-Free Read Lock

[![Crates.io](https://img.shields.io/crates/v/lfrlock)](https://crates.io/crates/lfrlock)
[![Documentation](https://docs.rs/lfrlock/badge.svg)](https://docs.rs/lfrlock)
[![License](https://img.shields.io/crates/l/lfrlock)](LICENSE-MIT)

A high-performance Lock-Free Read Lock implementation where reads never block and writes are serialized using a Mutex.

[中文文档](README_CN.md) | [English](README.md)

> **Note**: If you need a specialized Single-Writer Multiple-Reader (SWMR) version, please use [smr-swap](https://github.com/ShaoG-R/smr-swap) directly.

## Features

- **Lock-Free Reads**: Read operations are wait-free and never block, ensuring low latency.
- **Serialized Writes**: Write operations are serialized using a `Mutex` to prevent data races.
- **Unified Interface**: Supports both read and write operations through a single `LfrLock<T>` type, similar to `std::sync::Mutex`.
- **Easy Usage**: Provides a `WriteGuard` for familiar, mutable access that automatically commits changes on drop.
- **Safe Concurrency**: Built on top of `smr-swap` for safe memory reclamation and concurrent access.

## No-std Support

`lfrlock` supports `no_std` environments. To use it in a `no_std` crate:

1.  Disable default features.
2.  Enable the `spin` feature if you need Mutex support (which `LfrLock` uses for writes).
3.  Ensure `alloc` is available.

```toml
[dependencies]
lfrlock = { version = "0.2", default-features = false, features = ["spin"] }
```

Note: `LfrLock` relies on a Mutex for serializing writes. In `std` environments, it uses `std::sync::Mutex`. In `no_std` environments with the `spin` feature enabled, it uses `spin::Mutex`.

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
lfrlock = "0.2"
```

### Basic Usage

```rust
use lfrlock::LfrLock;
use std::thread;

#[derive(Debug, Clone)]
struct Data {
    value: i32,
}

fn main() {
    // Create a new LfrLock
    let lock = LfrLock::new(Data { value: 0 });

    let lock_clone = lock.clone();
    let handle = thread::spawn(move || {
        // Read data (never blocks)
        let data = lock_clone.read();
        println!("Reader sees: {}", data.value);
    });

    // Write data using WriteGuard (serialized)
    {
        let mut guard = lock.write();
        guard.value = 42;
    } // Auto-commit on drop

    handle.join().unwrap();
    
    let data = lock.read();
    println!("Final value: {}", data.value);
}
```

### Shared Access (Multi-threaded)

Since `LfrLock` is not `Sync` (it contains a thread-local reader), it cannot be shared via `Arc<LfrLock>`. Instead, obtain a `LfrLockFactory` from the lock (or create one directly), which is `Sync` and `Clone`.

```rust
use lfrlock::LfrLock;
use std::sync::Arc;
use std::thread;

fn main() {
    // Create a lock in the main thread
    let lock = LfrLock::new(0);
    
    // Create a factory for sharing (Sync + Clone)
    let factory = lock.factory();
    let factory = Arc::new(factory);

    let mut handles = vec![];

    for i in 0..4 {
        let factory = factory.clone();
        handles.push(thread::spawn(move || {
            // Create a thread-local lock instance
            let lock = factory.create();
            let val = lock.read();
            println!("Thread {} sees: {}", i, *val);
        }));
    }

    // Main thread can still use 'lock'
    lock.store(1);

    for h in handles {
        h.join().unwrap();
    }
}
```

## API Overview

### `LfrLock<T>`

The main type combining reader and writer capabilities.

#### Creation

- **`new(initial: T)`**: Creates a new lock with an initial value.
- **`From<T>`**: Supports `LfrLock::from(value)` or `value.into()`.
- **`Default`**: When `T: Default`, supports `LfrLock::default()`.

#### Read Operations

- **`read() -> ReadGuard<T>`**: Gets a lock-free read guard. Never blocks.
- **`get() -> T`**: Clones and returns the current value. Requires `T: Clone`.
- **`map<F, U>(f: F) -> U`**: Applies a closure to the current value and returns the transformed result.
- **`filter<F>(f: F) -> Option<ReadGuard<T>>`**: Conditional read, returns `Some(guard)` if closure returns `true`.
- **`factory() -> LfrLockFactory<T>`**: Creates a factory for sharing the lock across threads.

#### Write Operations

- **`store(new_value: T)`**: Directly replaces the current value.
- **`swap(new_value: T) -> T`**: Atomically swaps and returns the old value. Requires `T: Clone`.
- **`update<F>(f: F)`**: Updates data using a closure `FnOnce(&T) -> T`.
- **`update_and_fetch<F>(f: F) -> ReadGuard<T>`**: Updates and returns a guard to the new value.
- **`fetch_and_update<F>(f: F) -> ReadGuard<T>`**: Returns a guard to the old value and updates.
- **`write() -> WriteGuard<T>`**: Acquires a write lock and returns a guard for mutable access. Requires `T: Clone`.
- **`try_write() -> Option<WriteGuard<T>>`**: Tries to acquire the write lock.

### `LfrLockFactory<T>`

A factory for creating `LfrLock` instances. `Sync` and `Clone`, suitable for sharing across threads.

- **`new(initial: T)`**: Creates a new factory with an initial value.
- **`create() -> LfrLock<T>`**: Creates a new `LfrLock` handle for the current thread.

### `WriteGuard<T>`

Provides mutable access to the data.

- **Automatic Commit**: When the guard is dropped, the modified data is atomically swapped in.
- **Deref/DerefMut**: Access the underlying data transparently.

## Implementation Details

`LfrLock` uses `smr-swap` internally to manage state. It wraps the `Swapper` in a `Mutex` to serialize writes, while the `SwapReader` allows concurrent, lock-free reads. This design is ideal for read-heavy workloads where writes are infrequent but need to be safe and atomic.

## Performance Characteristics

Benchmark results comparing `LfrLock` against `ArcSwap` and `std::sync::Mutex` on an Intel(R) Core(TM) i9-13900KS CPU @ 3.20GHz.

### Benchmark Summary

| Scenario | LfrLock | ArcSwap | Mutex | Notes |
|----------|---------|---------|-------|-------|
| **Read Only (Single Thread)** | **0.86 ns** | 9.15 ns | 8.29 ns | **~10.6x faster** |
| **Read Heavy (Concurrent)** (1:1000) | **172 µs** | 220 µs | 1.92 ms | **~11.2x faster** than Mutex |
| **Read Heavy (Concurrent)** (1:100) | **195 µs** | 253 µs | 1.97 ms | **~1.3x faster** than ArcSwap |
| **Read Heavy (Concurrent)** (1:10) | **280 µs** | 553 µs | 2.26 ms | **~2.0x faster** than ArcSwap |
| **Write Heavy (Concurrent)** (16R:4W) | 3.22 ms | 3.28 ms | **1.15 ms** | Mutex ~2.8x faster |
| **Write Heavy (Concurrent)** (8R:4W) | 3.13 ms | 3.11 ms | **0.86 ms** | Mutex ~3.6x faster |
| **Write Heavy (Concurrent)** (4R:4W) | 3.13 ms | 3.04 ms | **0.78 ms** | Mutex ~4.0x faster |
| **Creation (new)** | 396 ns | 860 ns | **0.18 ns** | Mutex is instant |
| **Cloning** | 92 ns | **8.61 ns** | **8.62 ns** | LfrLock clone is heavier |

### Analysis

- **Read Performance**: `LfrLock` provides wait-free reads with nanosecond-scale latency (0.86ns), significantly outperforming `ArcSwap` and `Mutex` (~9ns).
- **High Contention Reads**: In mixed workloads (1:1000 to 1:10 write ratio), `LfrLock` maintains stable performance (~172-280µs), while `ArcSwap` degrades significantly at higher write rates (up to ~553µs).
- **Write Heavy**: `Mutex` is faster (~3-4x) in pure write-heavy scenarios because `LfrLock` involves RCU-like operations. `LfrLock` performance is comparable to `ArcSwap` in these scenarios.
- **Overhead**: `LfrLock` has higher cloning overhead (~92ns) compared to `Arc` cloning (~8.6ns) because it registers a new epoch reader. However, it is ~2.2x faster to create than `ArcSwap`.

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

