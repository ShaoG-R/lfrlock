# LfrLock: 无锁读取锁 (Lock-Free Read Lock)

[![Crates.io](https://img.shields.io/crates/v/lfrlock)](https://crates.io/crates/lfrlock)
[![Documentation](https://docs.rs/lfrlock/badge.svg)](https://docs.rs/lfrlock)
[![License](https://img.shields.io/crates/l/lfrlock)](LICENSE-MIT)

一个高性能的无锁读取锁实现，读取操作永不阻塞，写入操作通过 Mutex 进行串行化。

[中文文档](README_CN.md) | [English](README.md)

> **注意**：若需要 SWMR (单写多读) 特化版本，请直接使用 [smr-swap](https://github.com/ShaoG-R/smr-swap)。

## 特性

- **无锁读取**: 读取操作是无等待（wait-free）的且永不阻塞，确保低延迟。
- **串行化写入**: 写入操作使用 `Mutex` 串行化，防止数据竞争。
- **统一接口**: 通过单一的 `LfrLock<T>` 类型同时支持读写操作，类似于 `std::sync::Mutex`。
- **使用简便**: 提供 `WriteGuard` 用于习惯的可变访问，在 drop 时自动提交更改。
- **安全并发**: 基于 `smr-swap` 构建，确保安全的内存回收和并发访问。

## No-std 支持

`lfrlock` 支持 `no_std` 环境。要在 `no_std` crate 中使用它：

1.  禁用默认特性。
2.  如果需要 Mutex 支持（`LfrLock` 用于写入），请启用 `spin` 特性。
3.  确保 `alloc` 可用。

```toml
[dependencies]
lfrlock = { version = "0.2", default-features = false, features = ["spin"] }
```

注意：`LfrLock` 依赖 Mutex 来串行化写入。在 `std` 环境中，它使用 `std::sync::Mutex`。在启用了 `spin` 特性的 `no_std` 环境中，它使用 `spin::Mutex`。

## 快速开始

### 安装

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
lfrlock = "0.2"
```

### 基本用法

```rust
use lfrlock::LfrLock;
use std::thread;

#[derive(Debug, Clone)]
struct Data {
    value: i32,
}

fn main() {
    // 创建一个新的 LfrLock
    let lock = LfrLock::new(Data { value: 0 });

    let lock_clone = lock.clone();
    let handle = thread::spawn(move || {
        // 读取数据（永不阻塞）
        let data = lock_clone.read();
        println!("Reader sees: {}", data.value);
    });

    // 使用 WriteGuard 写入数据（串行化）
    {
        let mut guard = lock.write();
        guard.value = 42;
    } // drop 时自动提交

    handle.join().unwrap();
    
    let data = lock.read();
    println!("Final value: {}", data.value);
}
```

### 共享访问 (多线程)

由于 `LfrLock` 不是 `Sync` 的（它包含线程本地读取者），因此无法通过 `Arc<LfrLock>` 共享。请从锁实例获取 `LfrLockFactory`（或直接创建一个），它是 `Sync` 和 `Clone` 的。

```rust
use lfrlock::LfrLock;
use std::sync::Arc;
use std::thread;

fn main() {
    // 在主线程创建锁
    let lock = LfrLock::new(0);
    
    // 创建用于共享的工厂 (Sync + Clone)
    let factory = lock.factory();
    let factory = Arc::new(factory);

    let mut handles = vec![];

    for i in 0..4 {
        let factory = factory.clone();
        handles.push(thread::spawn(move || {
            // 创建线程本地的锁实例
            let lock = factory.create();
            let val = lock.read();
            println!("Thread {} sees: {}", i, *val);
        }));
    }

    // 主线程仍然可以使用 'lock'
    lock.store(1);

    for h in handles {
        h.join().unwrap();
    }
}
```

## API 概览

### `LfrLock<T>`

结合了读取者和写入者功能的主类型。

#### 创建

- **`new(initial: T)`**: 创建一个带有初始值的新锁。
- **`From<T>`**: 支持 `LfrLock::from(value)` 或 `value.into()`。
- **`Default`**: 当 `T: Default` 时，支持 `LfrLock::default()`。

#### 读取操作

- **`read() -> ReadGuard<T>`**: 获取无锁读取守卫。永不阻塞。
- **`get() -> T`**: 克隆并返回当前值。需要 `T: Clone`。
- **`map<F, U>(f: F) -> U`**: 对当前值应用闭包并返回转换结果。
- **`filter<F>(f: F) -> Option<ReadGuard<T>>`**: 条件读取，闭包返回 `true` 时返回 `Some(guard)`。
- **`factory() -> LfrLockFactory<T>`**: 创建一个在线程间共享锁的工厂。

#### 写入操作

- **`store(new_value: T)`**: 直接替换当前值。
- **`swap(new_value: T) -> T`**: 原子交换并返回旧值。需要 `T: Clone`。
- **`update<F>(f: F)`**: 使用闭包 `FnOnce(&T) -> T` 更新数据。
- **`update_and_fetch<F>(f: F) -> ReadGuard<T>`**: 更新并返回新值的守卫。
- **`fetch_and_update<F>(f: F) -> ReadGuard<T>`**: 返回旧值的守卫并更新。
- **`write() -> WriteGuard<T>`**: 获取写入锁并返回可变访问的守卫。需要 `T: Clone`。
- **`try_write() -> Option<WriteGuard<T>>`**: 尝试获取写入锁。

### `LfrLockFactory<T>`

用于创建 `LfrLock` 实例的工厂。`Sync` 且 `Clone`，适合跨线程共享。

- **`new(initial: T)`**: 创建一个带有初始值的新工厂。
- **`create() -> LfrLock<T>`**: 为当前线程创建一个新的 `LfrLock` 句柄。

### `WriteGuard<T>`

提供对数据的可变访问。

- **自动提交**: 当守卫被 drop 时，修改后的数据会被原子地换入。
- **Deref/DerefMut**: 透明地访问底层数据。

## 实现细节

`LfrLock` 内部使用 `smr-swap` 来管理状态。它将 `Swapper` 包裹在 `Mutex` 中以串行化写入，而 `SwapReader` 允许并发、无锁的读取。这种设计非常适合读多写少的场景，确保写入安全且原子化。

## 性能特性

在 Intel(R) Core(TM) i9-13900KS CPU @ 3.20GHz 上对比 `LfrLock`、`ArcSwap` 和 `std::sync::Mutex` 的基准测试结果。

### 基准测试摘要

| 场景 | LfrLock | ArcSwap | Mutex | 备注 |
|----------|---------|---------|-------|-------|
| **只读 (单线程)** | **0.86 ns** | 9.15 ns | 8.29 ns | **快约 10.6 倍** |
| **读密集 (并发)** (1:1000) | **172 µs** | 220 µs | 1.92 ms | 比 Mutex **快约 11.2 倍** |
| **读密集 (并发)** (1:100) | **195 µs** | 253 µs | 1.97 ms | 比 ArcSwap **快约 1.3 倍** |
| **读密集 (并发)** (1:10) | **280 µs** | 553 µs | 2.26 ms | 比 ArcSwap **快约 2.0 倍** |
| **写密集 (并发)** (16R:4W) | 3.22 ms | 3.28 ms | **1.15 ms** | Mutex 快约 2.8 倍 |
| **写密集 (并发)** (8R:4W) | 3.13 ms | 3.11 ms | **0.86 ms** | Mutex 快约 3.6 倍 |
| **写密集 (并发)** (4R:4W) | 3.13 ms | 3.04 ms | **0.78 ms** | Mutex 快约 4.0 倍 |
| **创建 (new)** | 396 ns | 860 ns | **0.18 ns** | Mutex 极快 |
| **克隆 (clone)** | 92 ns | **8.61 ns** | **8.62 ns** | LfrLock 克隆开销较大 |

### 分析

- **读取性能**: `LfrLock` 提供无等待读取，延迟仅为纳秒级 (0.86ns)，显著优于 `ArcSwap` and `Mutex` (~9ns)。
- **高并发读取**: 在混合负载（1:1000 到 1:10 写入比）中，`LfrLock` 保持稳定的性能 (~172-280µs)，而 `ArcSwap` 在高写入率下性能显著下降 (至 ~553µs)。
- **写密集**: 在纯写密集场景中，`Mutex` 更快 (~3-4倍)，因为 `LfrLock` 涉及 RCU 类操作。`LfrLock` 的性能在此类场景下与 `ArcSwap` 相当。
- **开销**: `LfrLock` 的克隆开销 (~92ns) 高于 `Arc` 克隆 (~8.6ns)，因为它需要注册新的 epoch 读取者。但在创建方面，它比 `ArcSwap` 快约 2.2 倍。

## 许可证

本项目采用以下任一许可证授权：

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

由你选择。
