use super::AgentError;
use druid::core::DruidError;
use serde::Deserialize;

/// JDBC Agent JSON-RPC 2.0 结构化错误。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentRpcError {
    code: i32,
    message: String,
    #[serde(default)]
    data: Option<AgentError>,
}

impl AgentRpcError {
    /// 转换为 Druid 驱动错误；JDBC 异常保留 `SQLState` 与 vendor code。
    #[must_use]
    pub fn into_druid_error(self) -> DruidError {
        self.data.map_or_else(
            || DruidError::Other(format!("JDBC Agent RPC {}: {}", self.code, self.message)),
            AgentError::into_druid_error,
        )
    }
}
