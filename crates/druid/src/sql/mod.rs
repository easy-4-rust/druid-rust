//! 基于 sqlparser-rs 的 SQL 解析兼容层与 Wall 规则。
//!
//! 对应 Druid Java 的 `com.alibaba.druid.wall` 和 `com.alibaba.druid.sql` 包。
//! SQL 解析替换为 sqlparser-rs（ADR-002），Wall 规则基于 AST 检查。

pub mod db_type;
pub mod jdbc_utils;
pub mod sql_utils;
pub mod wall;
pub mod wall_check_result;
pub mod wall_config;
pub mod wall_deny_stat;
pub mod wall_filter;
pub mod wall_function_stat;
pub mod wall_function_stat_value;
pub mod wall_provider;
pub mod wall_sql_function_stat;
pub mod wall_sql_stat;
pub mod wall_sql_stat_value;
pub mod wall_sql_table_stat;
pub mod wall_table_stat;
pub mod wall_table_stat_value;
pub mod wall_violation;

pub use db_type::DbType;
pub use jdbc_utils::JdbcUtils;
pub use sql_utils::{SqlFormatOption, SqlUtils};
pub use wall::Wall;
pub use wall_check_result::WallCheckResult;
pub use wall_config::{WallConfig, WallConfigBuilder};
pub use wall_deny_stat::WallDenyStat;
pub use wall_filter::WallFilter;
pub use wall_function_stat::WallFunctionStat;
pub use wall_function_stat_value::WallFunctionStatValue;
pub use wall_provider::WallProvider;
pub use wall_sql_function_stat::WallSqlFunctionStat;
pub use wall_sql_stat::WallSqlStat;
pub use wall_sql_stat_value::WallSqlStatValue;
pub use wall_sql_table_stat::WallSqlTableStat;
pub use wall_table_stat::WallTableStat;
pub use wall_table_stat_value::WallTableStatValue;
pub use wall_violation::WallViolation;
