//! Druid 核心与数据库适配器之间的低层 RDBC 合同。
//!
//! 此 crate 仅建立抽取边界；具体对象将在后续经兼容性门禁验证后迁移。

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
