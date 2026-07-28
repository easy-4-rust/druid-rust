//! 基于 sqlparser-rs 的 SQL 解析兼容层与 Wall 规则。
//!
//! 对应 Druid Java 的 `com.alibaba.druid.wall` 和 `com.alibaba.druid.sql` 包。
//! SQL 解析替换为 sqlparser-rs（ADR-002），Wall 规则基于 AST 检查。

pub mod wall;
pub mod wall_config;
pub mod wall_violation;

pub use wall::Wall;
pub use wall_config::{WallConfig, WallConfigBuilder};
pub use wall_violation::WallViolation;
