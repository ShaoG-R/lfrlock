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

Since v0.2.5, `LfrLock` defaults to the **Write-Preferred** strategy. The previous **Read-Preferred** strategy is now enabled via the `read-preferred` feature.

Benchmark results comparing `LfrLock` (both strategies), `ArcSwap`, and `std::sync::Mutex` on an Intel(R) Core(TM) i9-13900KS CPU @ 3.20GHz.

### Benchmark Summary

| Scenario | LfrLock (Write-Pref/Default) | LfrLock (Read-Pref) | ArcSwap | Mutex |
|----------|------------------------------|---------------------|---------|-------|
| **Read Only (Single Thread)** | **4.50 ns** | **0.86 ns** | 8.76 ns | 8.30 ns |
| **Read Heavy (Concurrent)** (1:1000) | **176 µs** | 172 µs | 216 µs | 1.94 ms |
| **Read Heavy (Concurrent)** (1:100) | **183 µs** | 195 µs | 252 µs | 2.01 ms |
| **Read Heavy (Concurrent)** (1:10) | **264 µs** | 280 µs | 583 µs | 2.24 ms |
| **Write Heavy (Concurrent)** (16R:4W) | **1.25 ms** | 3.22 ms | 3.08 ms | 1.30 ms |
| **Write Heavy (Concurrent)** (8R:4W) | 1.13 ms | 3.13 ms | 2.96 ms | **0.95 ms** |
| **Write Heavy (Concurrent)** (4R:4W) | 1.12 ms | 3.13 ms | 2.86 ms | **0.79 ms** |
| **Creation (new)** | 236 ns | 396 ns | 860 ns | **0.18 ns** |
| **Cloning** | 84 ns | 92 ns | **8.65 ns** | **8.64 ns** |

### Analysis

- **Default Write-Preferred Strategy**: While offering extremely fast reads (4.5ns), it significantly improves write performance. In mixed workloads and write-heavy scenarios, write performance is improved by about 2.5-3x compared to the Read-Preferred strategy, even outperforming `Mutex` in some high-concurrency scenarios.
- **Read-Preferred Strategy**: Still provides ultimate read performance (0.86ns), suitable for scenarios like configuration lists that are almost never updated.
- **Compared to ArcSwap**: Regardless of the strategy, `LfrLock` shows more stable performance in high-concurrency read scenarios. In Write-Preferred mode, write performance also significantly surpasses `ArcSwap`.
- **Compared to Mutex**: In read-heavy scenarios, `LfrLock` has an overwhelming advantage. In write-heavy scenarios, `LfrLock` with the Write-Preferred strategy can now compete with `Mutex`.

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

