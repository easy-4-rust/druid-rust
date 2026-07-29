use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 活跃连接查询响应。
///
/// 对应 Java: `com.alibaba.druid.admin.model.dto.ConnectionResult`。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionResult {
    /// Druid 管理协议结果码。
    #[serde(rename = "ResultCode")]
    pub result_code: i32,
    /// 活跃连接列表。
    #[serde(rename = "Content", default)]
    pub content: Option<Vec<ConnectionContent>>,
}

/// `ConnectionResult.ContentBean` 的 Rust 表达。
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ConnectionContent {
    pub id: i64,
    pub connection_id: i64,
    pub use_count: i64,
    pub last_active_time: Option<String>,
    pub connect_time: Option<String>,
    pub holdability: i32,
    pub transaction_isolation: i32,
    pub auto_commit: bool,
    /// 保留 Java JSON 中拼写错误的 `readoOnly` 字段。
    #[serde(rename = "readoOnly")]
    pub read_only: bool,
    pub keep_alive_check_count: i64,
    #[serde(default)]
    pub pscache: Option<Vec<Value>>,
}
