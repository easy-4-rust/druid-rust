use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 数据源统计查询响应。
///
/// 对应 Java: `com.alibaba.druid.admin.model.dto.DataSourceResult`。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataSourceResult {
    #[serde(rename = "ResultCode")]
    pub result_code: i32,
    #[serde(rename = "Content", default)]
    pub content: Option<Vec<DataSourceContent>>,
}

/// `DataSourceResult.ContentBean` 的 Rust 表达。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataSourceContent {
    #[serde(rename = "serviceId", skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    #[serde(rename = "Identity")]
    pub identity: i64,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "DbType")]
    pub db_type: Option<String>,
    #[serde(rename = "DriverClassName")]
    pub driver_class_name: Option<String>,
    #[serde(rename = "URL")]
    pub url: Option<String>,
    #[serde(rename = "UserName")]
    pub user_name: Option<String>,
    #[serde(rename = "WaitThreadCount")]
    pub wait_thread_count: i64,
    #[serde(rename = "NotEmptyWaitCount")]
    pub not_empty_wait_count: i64,
    #[serde(rename = "NotEmptyWaitMillis")]
    pub not_empty_wait_millis: i64,
    #[serde(rename = "PoolingCount")]
    pub pooling_count: i64,
    #[serde(rename = "PoolingPeak")]
    pub pooling_peak: i64,
    #[serde(rename = "PoolingPeakTime")]
    pub pooling_peak_time: Option<String>,
    #[serde(rename = "ActiveCount")]
    pub active_count: i64,
    #[serde(rename = "ActivePeak")]
    pub active_peak: i64,
    #[serde(rename = "ActivePeakTime")]
    pub active_peak_time: Option<String>,
    #[serde(rename = "InitialSize")]
    pub initial_size: i64,
    #[serde(rename = "MinIdle")]
    pub min_idle: i64,
    #[serde(rename = "MaxActive")]
    pub max_active: i64,
    #[serde(rename = "QueryTimeout")]
    pub query_timeout: i64,
    #[serde(rename = "TransactionQueryTimeout")]
    pub transaction_query_timeout: i64,
    #[serde(rename = "LoginTimeout")]
    pub login_timeout: i64,
    #[serde(rename = "ValidConnectionCheckerClassName")]
    pub valid_connection_checker_class_name: Option<String>,
    #[serde(rename = "ExceptionSorterClassName")]
    pub exception_sorter_class_name: Option<String>,
    #[serde(rename = "TestOnBorrow")]
    pub test_on_borrow: bool,
    #[serde(rename = "TestOnReturn")]
    pub test_on_return: bool,
    #[serde(rename = "TestWhileIdle")]
    pub test_while_idle: bool,
    #[serde(rename = "DefaultAutoCommit")]
    pub default_auto_commit: bool,
    #[serde(rename = "DefaultReadOnly")]
    pub default_read_only: Value,
    #[serde(rename = "DefaultTransactionIsolation")]
    pub default_transaction_isolation: Value,
    #[serde(rename = "LogicConnectCount")]
    pub logic_connect_count: i64,
    #[serde(rename = "LogicCloseCount")]
    pub logic_close_count: i64,
    #[serde(rename = "LogicConnectErrorCount")]
    pub logic_connect_error_count: i64,
    #[serde(rename = "PhysicalConnectCount")]
    pub physical_connect_count: i64,
    #[serde(rename = "PhysicalCloseCount")]
    pub physical_close_count: i64,
    #[serde(rename = "PhysicalConnectErrorCount")]
    pub physical_connect_error_count: i64,
    #[serde(rename = "ExecuteCount")]
    pub execute_count: i64,
    #[serde(rename = "ExecuteUpdateCount")]
    pub execute_update_count: i64,
    #[serde(rename = "ExecuteQueryCount")]
    pub execute_query_count: i64,
    #[serde(rename = "ExecuteBatchCount")]
    pub execute_batch_count: i64,
    #[serde(rename = "ErrorCount")]
    pub error_count: i64,
    #[serde(rename = "CommitCount")]
    pub commit_count: i64,
    #[serde(rename = "RollbackCount")]
    pub rollback_count: i64,
    #[serde(rename = "PSCacheAccessCount")]
    pub ps_cache_access_count: i64,
    #[serde(rename = "PSCacheHitCount")]
    pub ps_cache_hit_count: i64,
    #[serde(rename = "PSCacheMissCount")]
    pub ps_cache_miss_count: i64,
    #[serde(rename = "StartTransactionCount")]
    pub start_transaction_count: i64,
    #[serde(rename = "RemoveAbandoned")]
    pub remove_abandoned: bool,
    #[serde(rename = "ClobOpenCount")]
    pub clob_open_count: i64,
    #[serde(rename = "BlobOpenCount")]
    pub blob_open_count: i64,
    #[serde(rename = "KeepAliveCheckCount")]
    pub keep_alive_check_count: i64,
    #[serde(rename = "KeepAlive")]
    pub keep_alive: bool,
    #[serde(rename = "FailFast")]
    pub fail_fast: bool,
    #[serde(rename = "MaxWait")]
    pub max_wait: i64,
    #[serde(rename = "MaxWaitThreadCount")]
    pub max_wait_thread_count: i64,
    #[serde(rename = "PoolPreparedStatements")]
    pub pool_prepared_statements: bool,
    #[serde(rename = "MaxPoolPreparedStatementPerConnectionSize")]
    pub max_pool_prepared_statement_per_connection_size: i64,
    #[serde(rename = "MinEvictableIdleTimeMillis")]
    pub min_evictable_idle_time_millis: i64,
    #[serde(rename = "MaxEvictableIdleTimeMillis")]
    pub max_evictable_idle_time_millis: i64,
    #[serde(rename = "LogDifferentThread")]
    pub log_different_thread: bool,
    #[serde(rename = "RecycleErrorCount")]
    pub recycle_error_count: i64,
    #[serde(rename = "PreparedStatementOpenCount")]
    pub prepared_statement_open_count: i64,
    #[serde(rename = "PreparedStatementClosedCount")]
    pub prepared_statement_closed_count: i64,
    #[serde(rename = "UseUnfairLock")]
    pub use_unfair_lock: bool,
    #[serde(rename = "InitGlobalVariants")]
    pub init_global_variants: bool,
    #[serde(rename = "InitVariants")]
    pub init_variants: bool,
    #[serde(rename = "FilterClassNames", default)]
    pub filter_class_names: Option<Vec<String>>,
    #[serde(rename = "TransactionHistogram", default)]
    pub transaction_histogram: Option<Vec<i64>>,
    #[serde(rename = "ConnectionHoldTimeHistogram", default)]
    pub connection_hold_time_histogram: Option<Vec<i64>>,
}
