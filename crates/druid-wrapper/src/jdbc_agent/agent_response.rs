use super::AgentError;
use serde::Deserialize;
use serde_json::Value as JsonValue;

/// Druid JDBC Agent Protocol v1 响应帧。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentResponse {
    protocol_version: u32,
    request_id: u64,
    success: bool,
    #[serde(default)]
    payload: Option<JsonValue>,
    #[serde(default)]
    error: Option<AgentError>,
}

impl AgentResponse {
    pub(crate) const fn protocol_version(&self) -> u32 {
        self.protocol_version
    }

    pub(crate) const fn request_id(&self) -> u64 {
        self.request_id
    }

    pub(crate) const fn success(&self) -> bool {
        self.success
    }

    pub(crate) fn take_payload(&mut self) -> Option<JsonValue> {
        self.payload.take()
    }

    pub(crate) fn take_error(&mut self) -> Option<AgentError> {
        self.error.take()
    }
}
