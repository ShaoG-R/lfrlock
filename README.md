# LfrLock: Lock-Free Read Lock

[![Crates.io](https://img.shields.io/crates/v/lfrlock)](https://crates.io/crates/lfrlock)
[![Documentation](https://docs.rs/lfrlock/badge.svg)](https://docs.rs/lfrlock)
[![License](https://img.shields.io/crates/l/lfrlock)](LICENSE)

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

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.
