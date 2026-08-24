//! Druid 核心与数据库适配器之间的低层 RDBC 合同。
//!
//! 此 crate 仅建立抽取边界；具体对象将在后续经兼容性门禁验证后迁移。
//!
//! # 过渡标准化状态
//!
//! P03 status: TRANSITIONAL_PHYSICAL_SURFACE_PENDING_STANDARDIZATION
//!
//! 当前 `spi::Physical*` 与 resource runtime access surface 只是 P04 的标准化输入，
//! 不是 ADR-RDBC-006 所接受的最终 API。P04 Task 1 必须先退出
//! `compat::DruidError` waiver，之后才能将 JDBC 语义合并为单一 standard trait tree；
//! 本 crate 在 P03 不重命名、移动或删除这些对象。

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
