//! Java Druid `druid-wrapper` 模块的 Rust 适配门面。
//!
//! 对应 Java 模块：`/druid-wrapper`。Java 模块包装 c3p0、DBCP、Proxool；
//! Rust 迁移按等价职责聚合 RBDC、SQLx、bb8 与 deadpool Adapter。各 Adapter
//! 仍保留独立 crate，本 crate 只提供统一、无池中池的模块边界。

/// RBDC 直连驱动 Adapter。
pub use druid_rbdc as rbdc;
/// SQLx 直连驱动 Adapter。
pub use druid_sqlx as sqlx;
/// SQLx bb8 外部池桥接。
pub use druid_sqlx_bb8 as sqlx_bb8;
/// SQLx deadpool 外部池桥接。
pub use druid_sqlx_deadpool as sqlx_deadpool;
