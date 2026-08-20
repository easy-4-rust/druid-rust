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
/// Crate 内部兼容层：旧 `rdbc::*` 名称仅供迁移期内部引用。
///
/// 从 2026 Q3 起，与 Java `java.sql.*` 对标的标准类型请统一通过
/// `druid::sql::*` 访问，例如 `druid::sql::Connection / druid::sql::Statement /
/// druid::sql::ResultSet / druid::sql::SQLException / druid::sql::DataSource`。
///
/// 本模块内容与 `sql::*` 完全等价，但不会从 crate root 暴露；外部代码只能使用
/// `druid::sql::*`。保留该模块只是为了让尚未清理完的 crate 内引用可以渐进迁移。
#[path = "rdbc.rs"]
pub(crate) mod rdbc;
/// Public driver extension points for connection-bound RDBC resources.
pub mod spi;
/// SQL 解析与 Wall 实现。
pub mod sql;
/// Druid 统计实现。
pub mod stats;
/// 内置 Toasty 标准数据源实现。
pub mod toasty;
