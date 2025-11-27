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

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
lfrlock = "0.1"
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

## API Overview

### `LfrLock<T>`

The main type combining reader and writer capabilities.

- **`new(initial: T)`**: Creates a new lock with an initial value.
- **`read() -> ReaderGuard<T>`**: Gets a lock-free read guard. Never blocks.
- **`write() -> WriteGuard<T>`**: Acquires a write lock (blocks other writers) and returns a guard for mutable access. Requires `T: Clone`.
- **`write_with(f: F)`**: Updates data using a closure `FnOnce(&T) -> T`. Useful when `T` is not `Clone` or for functional updates.
- **`update(new_t: T)`**: Directly replaces the current value.
- **`try_write() -> Option<WriteGuard<T>>`**: Tries to acquire the write lock.

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
| **Read Only (Single Thread)** | **0.75 ns** | 9.33 ns | 8.48 ns | **~12.4x faster** |
| **Read Heavy (Concurrent)** (1:1000) | **180 µs** | 236 µs | 1.89 ms | **~10.5x faster** than Mutex |
| **Read Heavy (Concurrent)** (1:100) | **179 µs** | 267 µs | 1.94 ms | **~1.5x faster** than ArcSwap |
| **Read Heavy (Concurrent)** (1:10) | **220 µs** | 574 µs | 2.08 ms | **~2.6x faster** than ArcSwap |
| **Write Heavy (Concurrent)** (16R:4W) | 1.31 ms | 3.20 ms | **1.27 ms** | Mutex slightly faster |
| **Write Heavy (Concurrent)** (8R:4W) | 1.14 ms | 3.05 ms | **0.94 ms** | Mutex ~18% faster |
| **Write Heavy (Concurrent)** (4R:4W) | 1.15 ms | 2.96 ms | **0.76 ms** | Mutex ~34% faster |
| **Creation (new)** | 282 ns | 909 ns | **0.19 ns** | Mutex is instant |
| **Cloning** | 94 ns | **8.75 ns** | **8.80 ns** | LfrLock clone is heavier |

### Analysis

- **Read Performance**: `LfrLock` provides wait-free reads with nanosecond-scale latency (0.75ns), significantly outperforming `ArcSwap` and `Mutex` (~9ns).
- **High Contention Reads**: In mixed workloads (1:1000 to 1:10 write ratio), `LfrLock` maintains stable performance (~180-220µs), while `ArcSwap` degrades significantly at higher write rates (up to ~574µs).
- **Write Heavy**: `Mutex` is faster (~18-34%) in pure write-heavy scenarios because `LfrLock` involves RCU-like operations. `ArcSwap` is significantly slower.
- **Overhead**: `LfrLock` has higher cloning overhead (~94ns) compared to `Arc` cloning (~9ns) because it registers a new epoch reader. However, it is ~3.2x faster to create than `ArcSwap`.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
