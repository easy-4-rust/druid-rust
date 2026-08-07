use super::RdbcSqlStatValue;
use serde::Serialize;

/// 数据源区间统计的不可变管理快照。
///
/// 对应 Java：`com.alibaba.druid.pool.DruidDataSourceStatValue`。字符串类 Java
/// 可空字段使用 `Option`，不会用空字符串冒充未知元数据；计数、峰值和直方图均
/// 来自池与对外连接的真实生产路径。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct DruidDataSourceStatValue {
    pub name: String,
    pub db_type: Option<String>,
    pub driver_class_name: String,
    pub url: Option<String>,
    pub user_name: Option<String>,
    pub filter_class_names: Vec<String>,
    pub remove_abandoned: bool,
    pub initial_size: usize,
    pub min_idle: usize,
    pub max_active: usize,
    pub query_timeout: i32,
    pub transaction_query_timeout: i32,
    pub login_timeout: i32,
    pub valid_connection_checker_class_name: Option<String>,
    pub exception_sorter_class_name: Option<String>,
    pub test_on_borrow: bool,
    pub test_on_return: bool,
    pub test_while_idle: bool,
    pub default_auto_commit: bool,
    pub default_read_only: bool,
    pub default_transaction_isolation: Option<u8>,
    pub active_count: usize,
    pub active_peak: usize,
    pub active_peak_time: Option<u64>,
    pub pooling_count: usize,
    pub pooling_peak: usize,
    pub pooling_peak_time: Option<u64>,
    pub connect_count: u64,
    pub close_count: u64,
    pub wait_thread_count: usize,
    pub not_empty_wait_count: u64,
    pub not_empty_wait_nanos: u64,
    pub logic_connect_error_count: u64,
    pub physical_connect_count: u64,
    pub physical_close_count: u64,
    pub physical_connect_error_count: u64,
    pub execute_count: u64,
    pub error_count: u64,
    pub commit_count: u64,
    pub rollback_count: u64,
    pub pstmt_cache_hit_count: u64,
    pub pstmt_cache_miss_count: u64,
    pub start_transaction_count: u64,
    pub keep_alive_check_count: u64,
    pub connection_hold_time_histogram: [u64; 8],
    #[serde(rename = "Txn_0_1")]
    pub txn_0_1: u64,
    #[serde(rename = "Txn_1_10")]
    pub txn_1_10: u64,
    #[serde(rename = "Txn_10_100")]
    pub txn_10_100: u64,
    #[serde(rename = "Txn_100_1000")]
    pub txn_100_1000: u64,
    #[serde(rename = "Txn_1000_10000")]
    pub txn_1000_10000: u64,
    #[serde(rename = "Txn_10000_100000")]
    pub txn_10000_100000: u64,
    #[serde(rename = "Txn_more")]
    pub txn_more: u64,
    pub clob_open_count: u64,
    pub blob_open_count: u64,
    pub sql_skip_count: u64,
    pub sql_list: Vec<RdbcSqlStatValue>,
}

impl DruidDataSourceStatValue {
    /// 返回 `notEmptyWaitNanos` 截断到毫秒的值。
    ///
    /// 对应 Java：`DruidDataSourceStatValue#getNotEmptyWaitMillis()`。
    #[must_use]
    pub const fn not_empty_wait_millis(&self) -> u64 {
        self.not_empty_wait_nanos / 1_000_000
    }
}
