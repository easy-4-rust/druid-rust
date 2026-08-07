//! Java Druid `core` 模块的 Rust 语义迁移。
//!
//! 对应 Java 模块：`/core`。连接 SPI、连接池、SQL/Wall、统计、动态数据源和
//! 默认 Toasty 数据源都位于本 crate 的具名内部模块中。

/// Druid 公共对象、连接 SPI 与 Filter 基础。
pub mod core;
/// 高可用数据源与路由实现。
pub mod dynamic;
/// Druid 原生连接池实现。
pub mod pool;
/// 与 Java RDBC 4.2 命名、职责和错误语义对齐的 Rust 数据库连接门面。
#[path = "rdbc.rs"]
pub mod rdbc;
/// SQL 解析与 Wall 实现。
pub mod sql;
/// Druid 统计实现。
pub mod stats;
/// 内置 Toasty 标准数据源实现。
pub mod toasty;
