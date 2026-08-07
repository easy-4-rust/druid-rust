use serde::Serialize;

/// 单条 RDBC SQL 的不可变管理快照。
///
/// 对应 Java：`com.alibaba.druid.stat.RdbcSqlStatValue`。
#[derive(Debug, Clone, Default, Serialize)]
pub struct RdbcSqlStatValue {
    #[serde(rename = "ID")]
    pub id: u64,
    #[serde(rename = "SQL")]
    pub sql: String,
    #[serde(rename = "DataSource")]
    pub data_source: Option<String>,
    #[serde(rename = "HASH")]
    pub hash: i64,
    #[serde(rename = "ExecuteCount")]
    pub execute_count: u64,
    #[serde(rename = "TotalTime")]
    pub total_time_millis: u64,
    #[serde(rename = "LastTime")]
    pub last_time_millis: Option<u64>,
    #[serde(rename = "MaxTimespan")]
    pub max_timespan_millis: u64,
    #[serde(rename = "MaxTimespanOccurTime")]
    pub max_timespan_occur_time_millis: Option<u64>,
    #[serde(rename = "Histogram")]
    pub execute_time_histogram: [u64; 8],
    #[serde(rename = "ErrorCount")]
    pub error_count: u64,
    #[serde(rename = "BatchSizeTotal")]
    pub execute_batch_size_total: u64,
    #[serde(rename = "BatchSizeMax")]
    pub execute_batch_size_max: u64,
    #[serde(rename = "EffectedRowCount")]
    pub update_count: u64,
    #[serde(rename = "EffectedRowCountMax")]
    pub update_count_max: u64,
    #[serde(rename = "EffectedRowCountHistogram")]
    pub update_count_histogram: [u64; 6],
    #[serde(rename = "FetchRowCount")]
    pub fetch_row_count: u64,
    #[serde(rename = "FetchRowCountMax")]
    pub fetch_row_count_max: u64,
    #[serde(rename = "FetchRowCountHistogram")]
    pub fetch_row_count_histogram: [u64; 6],
    #[serde(rename = "RunningCount")]
    pub running_count: u64,
    #[serde(rename = "ConcurrentMax")]
    pub concurrent_max: u64,
    #[serde(rename = "InTransactionCount")]
    pub in_transaction_count: u64,
    #[serde(rename = "ResultSetHoldTime")]
    pub result_set_hold_time_millis: u64,
    #[serde(rename = "ExecuteAndResultSetHoldTime")]
    pub execute_and_result_set_hold_time_millis: u64,
    #[serde(rename = "ExecuteAndResultHoldTimeHistogram")]
    pub execute_and_result_hold_time_histogram: [u64; 8],
    #[serde(rename = "LastSlowParameters")]
    pub last_slow_parameters: Option<String>,
    #[serde(rename = "LastErrorMessage")]
    pub last_error_message: Option<String>,
    #[serde(rename = "LastError")]
    pub last_error: Option<serde_json::Value>,
    #[serde(rename = "LastErrorClass")]
    pub last_error_class: Option<String>,
    #[serde(rename = "LastErrorStackTrace")]
    pub last_error_stack_trace: Option<String>,
    #[serde(rename = "LastErrorTime")]
    pub last_error_time_millis: Option<u64>,
    #[serde(rename = "ReadStringLength")]
    pub read_string_length: u64,
    #[serde(rename = "ReadBytesLength")]
    pub read_bytes_length: u64,
    #[serde(rename = "InputStreamOpenCount")]
    pub input_stream_open_count: u64,
    #[serde(rename = "ReaderOpenCount")]
    pub reader_open_count: u64,
    #[serde(rename = "ClobOpenCount")]
    pub clob_open_count: u64,
    #[serde(rename = "BlobOpenCount")]
    pub blob_open_count: u64,
    #[serde(rename = "Name")]
    pub name: Option<String>,
    #[serde(rename = "File")]
    pub file: Option<String>,
    #[serde(rename = "DbType")]
    pub db_type: Option<String>,
    #[serde(rename = "URL")]
    pub url: Option<String>,
}
