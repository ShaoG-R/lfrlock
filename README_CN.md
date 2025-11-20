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

## 快速开始

### 安装

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
lfrlock = "0.1"
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

## API 概览

### `LfrLock<T>`

结合了读取者和写入者功能的主类型。

- **`new(initial: T)`**: 创建一个带有初始值的新锁。
- **`read() -> ReaderGuard<T>`**: 获取无锁读取守卫。永不阻塞。
- **`write() -> WriteGuard<T>`**: 获取写入锁（阻塞其他写入者）并返回用于可变访问的守卫。需要 `T: Clone`。
- **`write_with(f: F)`**: 使用闭包 `FnOnce(&T) -> T` 更新数据。当 `T` 未实现 `Clone` 或进行函数式更新时很有用。
- **`update(new_t: T)`**: 直接替换当前值。
- **`try_write() -> Option<WriteGuard<T>>`**: 尝试获取写入锁。

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
| **只读 (单线程)** | **0.74 ns** | 8.77 ns | 8.35 ns | **快约 11.8 倍** |
| **读密集 (并发)** (1:1000) | **171 µs** | 218 µs | 1.87 ms | 比 Mutex **快约 10.9 倍** |
| **读密集 (并发)** (1:100) | **168 µs** | 253 µs | 1.84 ms | 比 ArcSwap **快约 1.5 倍** |
| **读密集 (并发)** (1:10) | **225 µs** | 593 µs | 2.10 ms | 比 ArcSwap **快约 2.6 倍** |
| **写密集 (并发)** (16R:4W) | 1.26 ms | 3.19 ms | **1.20 ms** | Mutex 略快 |
| **写密集 (并发)** (8R:4W) | 1.10 ms | 3.11 ms | **0.90 ms** | Mutex 快约 20% |
| **写密集 (并发)** (4R:4W) | 1.01 ms | 2.98 ms | **0.79 ms** | Mutex 快约 27% |
| **创建 (new)** | 231 ns | 912 ns | **0.19 ns** | Mutex 极快 |
| **克隆 (clone)** | 96 ns | **8.7 ns** | **8.7 ns** | LfrLock 克隆开销较大 |

### 分析

- **读取性能**: `LfrLock` 提供无等待读取，延迟仅为纳秒级 (0.74ns)，显著优于 `ArcSwap` 和 `Mutex` (~8ns)。
- **高并发读取**: 在混合负载（1:1000 到 1:10 写入比）中，`LfrLock` 保持稳定的性能 (~170-225µs)，而 `ArcSwap` 在高写入率下性能显著下降 (至 ~600µs)。
- **写密集**: 在纯写密集场景中，`Mutex` 略快 (~20%)，因为 `LfrLock` 涉及 RCU 类操作。`ArcSwap` 则明显较慢。
- **开销**: `LfrLock` 的克隆开销 (~98ns) 高于 `Arc` 克隆 (~9ns)，因为它需要注册新的 epoch 读取者。但在创建方面，它比 `ArcSwap` 快约 4 倍。

## 许可证

根据您的选择，根据 Apache License, Version 2.0 或 MIT 许可证进行许可。
