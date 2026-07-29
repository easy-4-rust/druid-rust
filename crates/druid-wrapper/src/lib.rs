//! Java Druid `druid-wrapper` 模块的 Rust 语义迁移。
//!
//! 对应 Java 模块：`/druid-wrapper`。Java 模块包装 c3p0、DBCP、Proxool；
//! Rust 迁移按等价职责在本 crate 内部聚合 RBDC、SQLx、bb8 与 deadpool Adapter。

mod prepared_parameter_materializer;
mod prepared_parameter_state;

/// RBDC 直连驱动 Adapter。
pub mod rbdc;
/// SQLx 直连驱动 Adapter。
pub mod sqlx;
/// SQLx bb8 外部池桥接。
pub mod sqlx_bb8;
/// SQLx deadpool 外部池桥接。
pub mod sqlx_deadpool;
