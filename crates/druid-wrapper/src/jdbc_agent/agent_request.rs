use serde::Serialize;
use serde_json::Value as JsonValue;

/// Druid JDBC Agent Protocol v1 请求帧。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRequest {
    protocol_version: u32,
    request_id: u64,
    operation: String,
    payload: JsonValue,
}

impl AgentRequest {
    /// 创建协议版本固定为 1 的请求。
    #[must_use]
    pub fn new(request_id: u64, operation: impl Into<String>, payload: JsonValue) -> Self {
        Self {
            protocol_version: 1,
            request_id,
            operation: operation.into(),
            payload,
        }
    }
}
