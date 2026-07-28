//! Java Druid `core` 模块的 Rust crate 门面。
//!
//! 对应 Java 模块：`/core`。本 crate 只固定 Java `core` 与 Rust 多个内部实现
//! crate 的模块边界，不复制对象，也不把尚未迁移的语义标记为完成。

/// Druid 公共对象、连接 SPI 与 Filter 基础。
pub use druid_core as core;
/// 高可用数据源与路由实现。
pub use druid_dynamic as dynamic;
/// Druid 原生连接池实现。
pub use druid_pool as pool;
/// SQL 解析与 Wall 实现。
pub use druid_sql as sql;
/// Druid 统计实现。
pub use druid_stats as stats;
/// 内置 Toasty 标准数据源实现。
pub use druid_toasty as toasty;
