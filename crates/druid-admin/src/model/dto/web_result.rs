use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Web URI 统计响应。
///
/// 对应 Java: `com.alibaba.druid.admin.model.dto.WebResult`。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebResult {
    #[serde(rename = "ResultCode")]
    pub result_code: i32,
    #[serde(rename = "Content", default)]
    pub content: Option<Vec<WebContent>>,
}

/// `WebResult.ContentBean` 的 Rust 表达。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WebContent {
    #[serde(rename = "URI")]
    pub uri: Option<String>,
    #[serde(rename = "RunningCount")]
    pub running_count: i64,
    #[serde(rename = "ConcurrentMax")]
    pub concurrent_max: i64,
    #[serde(rename = "RequestCount")]
    pub request_count: i64,
    #[serde(rename = "RequestTimeMillis")]
    pub request_time_millis: i64,
    #[serde(rename = "ErrorCount")]
    pub error_count: i64,
    #[serde(rename = "LastAccessTime")]
    pub last_access_time: Option<String>,
    #[serde(rename = "JdbcCommitCount")]
    pub jdbc_commit_count: i64,
    #[serde(rename = "JdbcRollbackCount")]
    pub jdbc_rollback_count: i64,
    #[serde(rename = "JdbcExecuteCount")]
    pub jdbc_execute_count: i64,
    #[serde(rename = "JdbcExecuteErrorCount")]
    pub jdbc_execute_error_count: i64,
    #[serde(rename = "JdbcExecutePeak")]
    pub jdbc_execute_peak: i64,
    #[serde(rename = "JdbcExecuteTimeMillis")]
    pub jdbc_execute_time_millis: i64,
    #[serde(rename = "JdbcFetchRowCount")]
    pub jdbc_fetch_row_count: i64,
    #[serde(rename = "JdbcFetchRowPeak")]
    pub jdbc_fetch_row_peak: i64,
    #[serde(rename = "JdbcUpdateCount")]
    pub jdbc_update_count: i64,
    #[serde(rename = "JdbcUpdatePeak")]
    pub jdbc_update_peak: i64,
    #[serde(rename = "JdbcPoolConnectionOpenCount")]
    pub jdbc_pool_connection_open_count: i64,
    #[serde(rename = "JdbcPoolConnectionCloseCount")]
    pub jdbc_pool_connection_close_count: i64,
    #[serde(rename = "JdbcResultSetOpenCount")]
    pub jdbc_result_set_open_count: i64,
    #[serde(rename = "JdbcResultSetCloseCount")]
    pub jdbc_result_set_close_count: i64,
    #[serde(rename = "RequestTimeMillisMax")]
    pub request_time_millis_max: i64,
    #[serde(rename = "RequestTimeMillisMaxOccurTime")]
    pub request_time_millis_max_occur_time: Option<String>,
    #[serde(rename = "Histogram", default)]
    pub histogram: Option<Vec<i64>>,
    #[serde(rename = "Profiles", default)]
    pub profiles: Option<Vec<Value>>,
}
