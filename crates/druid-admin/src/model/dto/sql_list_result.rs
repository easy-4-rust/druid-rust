use serde::{Deserialize, Serialize};
use serde_json::Value;

/// SQL 列表统计响应。
///
/// 对应 Java: `com.alibaba.druid.admin.model.dto.SqlListResult`。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SqlListResult {
    #[serde(rename = "ResultCode")]
    pub result_code: i32,
    #[serde(rename = "Content", default)]
    pub content: Option<Vec<SqlListContent>>,
}

/// `SqlListResult.ContentBean` 的 Rust 表达。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SqlListContent {
    #[serde(rename = "serviceId", skip_serializing_if = "Option::is_none")]
    pub service_id: Option<String>,
    #[serde(rename = "address", skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(rename = "port", skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(rename = "ExecuteAndResultSetHoldTime")]
    pub execute_and_result_set_hold_time: i64,
    #[serde(rename = "LastErrorMessage")]
    pub last_error_message: Value,
    #[serde(rename = "InputStreamOpenCount")]
    pub input_stream_open_count: i64,
    #[serde(rename = "BatchSizeTotal")]
    pub batch_size_total: i64,
    #[serde(rename = "FetchRowCountMax")]
    pub fetch_row_count_max: i64,
    #[serde(rename = "ErrorCount")]
    pub error_count: i64,
    #[serde(rename = "BatchSizeMax")]
    pub batch_size_max: i64,
    #[serde(rename = "URL")]
    pub url: Value,
    #[serde(rename = "Name")]
    pub name: Value,
    #[serde(rename = "LastErrorTime")]
    pub last_error_time: Value,
    #[serde(rename = "ReaderOpenCount")]
    pub reader_open_count: i64,
    #[serde(rename = "EffectedRowCountMax")]
    pub effected_row_count_max: i64,
    #[serde(rename = "LastErrorClass")]
    pub last_error_class: Value,
    #[serde(rename = "InTransactionCount")]
    pub in_transaction_count: i64,
    #[serde(rename = "LastErrorStackTrace")]
    pub last_error_stack_trace: Value,
    #[serde(rename = "ResultSetHoldTime")]
    pub result_set_hold_time: i64,
    #[serde(rename = "TotalTime")]
    pub total_time: i64,
    #[serde(rename = "ID")]
    pub id: i64,
    #[serde(rename = "ConcurrentMax")]
    pub concurrent_max: i64,
    #[serde(rename = "RunningCount")]
    pub running_count: i64,
    #[serde(rename = "FetchRowCount")]
    pub fetch_row_count: i64,
    #[serde(rename = "MaxTimespanOccurTime")]
    pub max_timespan_occur_time: Option<String>,
    #[serde(rename = "LastSlowParameters")]
    pub last_slow_parameters: Value,
    #[serde(rename = "ReadBytesLength")]
    pub read_bytes_length: i64,
    #[serde(rename = "DbType")]
    pub db_type: Option<String>,
    #[serde(rename = "DataSource")]
    pub data_source: Value,
    #[serde(rename = "SQL")]
    pub sql: Option<String>,
    #[serde(rename = "HASH")]
    pub hash: i64,
    #[serde(rename = "LastError")]
    pub last_error: Value,
    #[serde(rename = "MaxTimespan")]
    pub max_timespan: i64,
    #[serde(rename = "BlobOpenCount")]
    pub blob_open_count: i64,
    #[serde(rename = "ExecuteCount")]
    pub execute_count: i64,
    #[serde(rename = "EffectedRowCount")]
    pub effected_row_count: i64,
    #[serde(rename = "ReadStringLength")]
    pub read_string_length: i64,
    #[serde(rename = "File")]
    pub file: Value,
    #[serde(rename = "ClobOpenCount")]
    pub clob_open_count: i64,
    #[serde(rename = "LastTime")]
    pub last_time: Option<String>,
    #[serde(rename = "EffectedRowCountHistogram", default)]
    pub effected_row_count_histogram: Option<Vec<i64>>,
    #[serde(rename = "Histogram", default)]
    pub histogram: Option<Vec<i64>>,
    #[serde(rename = "ExecuteAndResultHoldTimeHistogram", default)]
    pub execute_and_result_hold_time_histogram: Option<Vec<i64>>,
    #[serde(rename = "FetchRowCountHistogram", default)]
    pub fetch_row_count_histogram: Option<Vec<i64>>,
}
