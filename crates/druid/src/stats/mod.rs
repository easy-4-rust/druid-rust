//! SQL 合并统计、百分位直方图与 Prometheus 导出。
//!
//! 对应 Druid Java 的 `com.alibaba.druid.stat` 包。

pub mod data_source_monitorable;
pub mod druid_data_source_stat_manager;
pub mod druid_data_source_stat_value;
pub mod druid_stat_manager_facade;
pub mod druid_stat_service;
pub mod merge;
pub mod merge_stat_filter;
pub mod rdbc_connection_stat;
pub mod rdbc_data_source_stat;
pub mod rdbc_result_set_stat;
pub mod rdbc_sql_stat;
pub mod rdbc_sql_stat_value;
pub mod rdbc_stat_context;
pub mod rdbc_stat_manager;
pub mod rdbc_statement_stat;
pub mod rdbc_trace_manager;
pub mod stat_filter;
pub mod stat_filter_context;
pub mod stat_filter_context_listener;
pub mod stat_filter_context_listener_adapter;
pub mod table_stat;

pub use data_source_monitorable::DataSourceMonitorable;
pub use druid_data_source_stat_manager::DruidDataSourceStatManager;
pub use druid_data_source_stat_value::DruidDataSourceStatValue;
pub use druid_stat_manager_facade::DruidStatManagerFacade;
pub use druid_stat_service::DruidStatService;
pub use rdbc_connection_stat::{
    RdbcConnectionStat, RdbcConnectionStatEntry, RdbcConnectionStatEntryValue,
};
pub use rdbc_data_source_stat::RdbcDataSourceStat;
/// 旧内部名称，保留源码兼容；canonical 对象为 [`RdbcDataSourceStat`]。
pub type StatsCollector = RdbcDataSourceStat;
pub use merge::{fingerprint, parameterize, MergedSqlStat, SqlMerger};
pub use merge_stat_filter::MergeStatFilter;
pub use rdbc_result_set_stat::RdbcResultSetStat;
pub use rdbc_sql_stat::RdbcSqlStat;
pub use rdbc_sql_stat_value::RdbcSqlStatValue;
pub use rdbc_stat_context::RdbcStatContext;
pub use rdbc_stat_manager::RdbcStatManager;
pub use rdbc_statement_stat::RdbcStatementStat;
#[allow(deprecated)]
pub use rdbc_trace_manager::RdbcTraceManager;
pub use stat_filter::StatFilter;
pub use stat_filter_context::StatFilterContext;
pub use stat_filter_context_listener::StatFilterContextListener;
pub use stat_filter_context_listener_adapter::StatFilterContextListenerAdapter;
pub use table_stat::{
    TableStat, TableStatColumn, TableStatCondition, TableStatMode, TableStatName,
    TableStatRelationship,
};
