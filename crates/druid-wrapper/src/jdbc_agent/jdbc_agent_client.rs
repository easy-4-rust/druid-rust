use super::{AgentRequestHandle, AgentRuntimeLease, JdbcAgentOptions};
use druid_core::core::DruidError;
use serde_json::{json, Value as JsonValue};

/// 共享 JDBC Agent 运行时中的单个数据库 session 客户端。
pub(crate) struct JdbcAgentClient {
    runtime: AgentRuntimeLease,
    session_id: String,
    closed: bool,
}

impl JdbcAgentClient {
    /// 取得共享 Agent 进程并建立隔离 session。
    pub(crate) async fn connect(
        options: JdbcAgentOptions,
        connect_payload: JsonValue,
    ) -> Result<(Self, JsonValue), DruidError> {
        let runtime = super::agent_runtime_manager::AgentRuntimeManager::acquire(&options).await?;
        let result = runtime.request("open_session", connect_payload).await?;
        let session_id = result
            .get("sessionId")
            .and_then(JsonValue::as_str)
            .filter(|session_id| !session_id.is_empty())
            .ok_or_else(|| {
                DruidError::DriverError(
                    "JDBC Agent open_session did not return sessionId".to_owned(),
                )
            })?
            .to_owned();
        Ok((
            Self {
                runtime,
                session_id,
                closed: false,
            },
            result,
        ))
    }

    /// 在当前 session 中执行一次请求。
    pub(crate) async fn request(
        &mut self,
        operation: &str,
        payload: JsonValue,
    ) -> Result<JsonValue, DruidError> {
        if self.closed {
            return Err(DruidError::ConnectionDiscarded);
        }
        let mut params = match payload {
            JsonValue::Object(object) => object,
            JsonValue::Null => serde_json::Map::new(),
            value => {
                let mut object = serde_json::Map::new();
                object.insert("value".to_owned(), value);
                object
            }
        };
        params.insert(
            "sessionId".to_owned(),
            JsonValue::String(self.session_id.clone()),
        );
        self.runtime
            .request(operation, JsonValue::Object(params))
            .await
    }

    /// 仅关闭当前 session；共享 Agent 在空闲 TTL 到期后退出。
    pub(crate) async fn close(&mut self) -> Result<(), DruidError> {
        if self.closed {
            return Ok(());
        }
        let result = self
            .runtime
            .request("close_session", json!({"sessionId": self.session_id}))
            .await
            .map(|_| ());
        self.closed = true;
        self.runtime.release_now();
        result
    }

    pub(crate) fn is_unusable(&self) -> bool {
        self.closed || self.runtime.is_unusable()
    }

    /// 返回 Statement 同步 close/cancel 可使用的轻量运行时句柄和 session ID。
    pub(crate) fn statement_context(&self) -> (AgentRequestHandle, String) {
        (self.runtime.request_handle(), self.session_id.clone())
    }
}
