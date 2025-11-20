# LfrLock: 无锁读取锁 (Lock-Free Read Lock)

[![Crates.io](https://img.shields.io/crates/v/lfrlock)](https://crates.io/crates/lfrlock)
[![Documentation](https://docs.rs/lfrlock/badge.svg)](https://docs.rs/lfrlock)
[![License](https://img.shields.io/crates/l/lfrlock)](LICENSE)

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

## 许可证

根据您的选择，根据 Apache License, Version 2.0 或 MIT 许可证进行许可。
