use super::AgentRpcError;
use serde::Deserialize;
use serde_json::Value as JsonValue;

/// JDBC Agent JSON-RPC 2.0 响应。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentResponse {
    jsonrpc: String,
    id: u64,
    #[serde(default)]
    result: Option<JsonValue>,
    #[serde(default)]
    error: Option<AgentRpcError>,
}

impl AgentResponse {
    pub(crate) fn validate_version(&self) -> bool {
        self.jsonrpc == "2.0"
    }

    pub(crate) const fn request_id(&self) -> u64 {
        self.id
    }

    pub(crate) fn take_result(&mut self) -> Option<JsonValue> {
        self.result.take()
    }

    pub(crate) fn take_error(&mut self) -> Option<AgentRpcError> {
        self.error.take()
    }
}
