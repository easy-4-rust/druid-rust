//! SQL 合并统计、百分位直方图与 Prometheus 导出。
//!
//! 对应 Druid Java 的 `com.alibaba.druid.stat` 包。

pub mod data_source_monitorable;
pub mod druid_data_source_stat_manager;
pub mod druid_stat_manager_facade;
pub mod druid_stat_service;
pub mod jdbc_data_source_stat;
pub mod jdbc_result_set_stat;
pub mod jdbc_sql_stat;
pub mod jdbc_sql_stat_value;
pub mod merge;
pub mod merge_stat_filter;
pub mod stat_filter;
pub mod stat_filter_context;
pub mod stat_filter_context_listener;
pub mod stat_filter_context_listener_adapter;

pub use data_source_monitorable::DataSourceMonitorable;
pub use druid_data_source_stat_manager::DruidDataSourceStatManager;
pub use druid_stat_manager_facade::DruidStatManagerFacade;
pub use druid_stat_service::DruidStatService;
pub use jdbc_data_source_stat::JdbcDataSourceStat;
/// 旧内部名称，保留源码兼容；canonical 对象为 [`JdbcDataSourceStat`]。
pub type StatsCollector = JdbcDataSourceStat;
pub use jdbc_result_set_stat::JdbcResultSetStat;
pub use jdbc_sql_stat::JdbcSqlStat;
pub use jdbc_sql_stat_value::JdbcSqlStatValue;
pub use merge::{fingerprint, parameterize, MergedSqlStat, SqlMerger};
pub use merge_stat_filter::MergeStatFilter;
pub use stat_filter::StatFilter;
pub use stat_filter_context::StatFilterContext;
pub use stat_filter_context_listener::StatFilterContextListener;
pub use stat_filter_context_listener_adapter::StatFilterContextListenerAdapter;
