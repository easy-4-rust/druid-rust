//! JDBC 4.2 语义对齐的统一 Rust 数据库合同。
//!
//! `connection`、`statement`、`result_set`、`metadata`、`driver`、`data_source` 与
//! `wrapper` 构成唯一 canonical standard。迁移期 `spi::Physical*` 仅作为待合并输入保留，
//! 不代表第二套标准。
//!
//! # 过渡标准化状态
//!
//! P04 status: STANDARD_TRAITS_DECLARED_PHYSICAL_CUTOVER_PENDING
//!
//! 当前 `spi::Physical*` 与 resource runtime access surface 只是后续 adapter/logical cutover
//! 的迁移输入，不是 ADR-RDBC-006 所接受的第二套 API。本阶段不以 alias 伪造两套 method
//! set 的 type identity；旧名称保持 transitional，直至 Task 6 完成合并与删除。

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
/// 标准对象运行时识别与解包合同。
pub mod wrapper;

/// 迁移期 Java 语义兼容合同。
pub mod compat;
/// RDBC 错误与 SQL 异常合同。
pub mod error;
/// LOB、数组、引用和资源上下文合同。
pub mod resource;
/// 物理连接与语句的驱动 SPI。
pub mod spi;
/// 平台值、SQL 类型与日期时间合同。
pub mod types;

pub use error::RdbcError;
