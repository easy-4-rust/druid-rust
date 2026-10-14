//! JDBC 4.2 语义对齐的统一 Rust 数据库合同。
//!
//! `connection`、`statement`、`result_set`、`metadata`、`driver`、`data_source`、
//! `resource` 与 `wrapper` 构成唯一 canonical standard。数据库适配器与 Druid pooled
//! proxies 直接实现这些合同，不存在并行驱动 trait tree。

/// JDBC-like 连接合同。
pub mod connection;
/// JDBC-like 数据源合同。
pub mod data_source;
/// 驱动、连接工厂与注册表合同。
pub mod driver;
/// Rust 专用的标准扩展。
pub mod extensions;
/// JDBC-like metadata 合同。
pub mod metadata;
/// JDBC-like 游标合同。
pub mod result_set;
/// JDBC-like 语句合同与类型化描述符。
pub mod statement;
/// 产品档案连接 URL 解析合同。
pub mod url;
/// 标准对象运行时识别与解包合同。
pub mod wrapper;

/// 迁移期 Java 语义兼容合同。
pub mod compat;
/// RDBC 错误与 SQL 异常合同。
pub mod error;
/// LOB、数组、引用和资源上下文合同。
pub mod resource;
/// 平台值、SQL 类型与日期时间合同。
pub mod types;

pub use error::RdbcError;
