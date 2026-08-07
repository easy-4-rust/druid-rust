use serde::Serialize;
use serde_json::Value as JsonValue;

/// JDBC Agent JSON-RPC 2.0 请求。
#[derive(Debug, Serialize)]
pub struct AgentRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: JsonValue,
}

impl<'a> AgentRequest<'a> {
    /// 创建带关联 ID 的 JSON-RPC 请求。
    #[must_use]
    pub const fn new(id: u64, method: &'a str, params: JsonValue) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method,
            params,
        }
    }
}
