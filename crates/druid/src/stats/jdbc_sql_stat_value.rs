use serde::Serialize;

/// 单条 JDBC SQL 的不可变管理快照。
///
/// 对应 Java：`com.alibaba.druid.stat.JdbcSqlStatValue`。
#[derive(Debug, Clone, Default, Serialize)]
pub struct JdbcSqlStatValue {
    #[serde(rename = "ID")]
    pub id: u64,
    #[serde(rename = "SQL")]
    pub sql: String,
    #[serde(rename = "HASH")]
    pub hash: u64,
    #[serde(rename = "ExecuteCount")]
    pub execute_count: u64,
    #[serde(rename = "TotalTime")]
    pub total_time_millis: u64,
    #[serde(rename = "MaxTimespan")]
    pub max_timespan_millis: u64,
    #[serde(rename = "ErrorCount")]
    pub error_count: u64,
    #[serde(rename = "FetchRowCount")]
    pub fetch_row_count: u64,
    #[serde(rename = "RunningCount")]
    pub running_count: u64,
    #[serde(rename = "ConcurrentMax")]
    pub concurrent_max: u64,
}
