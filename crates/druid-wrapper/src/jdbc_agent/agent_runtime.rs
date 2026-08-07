use super::{AgentRequest, AgentResponse, JdbcAgentOptions};
use druid::core::DruidError;
use serde_json::{json, Value as JsonValue};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// 一个可承载多个 JDBC session 的 Agent 子进程。
///
/// Rust 侧串行化标准输入输出交换；数据库连接隔离由协议中的 `sessionId`
/// 保证。对应 Java: `AgentServer` JSON-RPC 循环。
pub(crate) struct AgentRuntime {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_request_id: u64,
    frame_limit: usize,
}

impl AgentRuntime {
    /// 启动 Agent，校验 ready 通知并完成版本/能力握手。
    pub(crate) async fn start(options: &JdbcAgentOptions) -> Result<Self, DruidError> {
        let mut command = Command::new(options.program());
        command
            .args(options.arguments())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            DruidError::DriverError(format!("failed to spawn JDBC Agent: {error}"))
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| DruidError::DriverError("JDBC Agent stdin was not piped".to_owned()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| DruidError::DriverError("JDBC Agent stdout was not piped".to_owned()))?;
        let mut runtime = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_request_id: 1,
            frame_limit: options.frame_limit(),
        };
        let ready = runtime.read_json_line().await?;
        if ready.get("jsonrpc").and_then(JsonValue::as_str) != Some("2.0")
            || ready.get("method").and_then(JsonValue::as_str) != Some("ready")
            || ready
                .pointer("/params/protocolVersion")
                .and_then(JsonValue::as_u64)
                != Some(1)
        {
            return Err(DruidError::DriverError(format!(
                "JDBC Agent sent an invalid ready notification: {ready}"
            )));
        }
        runtime
            .request(
                "handshake",
                json!({
                    "protocolVersion": 1,
                    "client": "druid-rust",
                    "capabilities": ["multi-session", "structured-errors", "tagged-values"]
                }),
            )
            .await?;
        Ok(runtime)
    }

    /// 发送一个 JSON-RPC 请求并按 ID 关联响应。
    pub(crate) async fn request(
        &mut self,
        method: &str,
        params: JsonValue,
    ) -> Result<JsonValue, DruidError> {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        let request = AgentRequest::new(request_id, method, params);
        let mut bytes = serde_json::to_vec(&request).map_err(Self::protocol_error)?;
        self.validate_frame_size(bytes.len())?;
        bytes.push(b'\n');
        self.stdin.write_all(&bytes).await.map_err(Self::io_error)?;
        self.stdin.flush().await.map_err(Self::io_error)?;

        let response_bytes = self.read_line().await?;
        let mut response: AgentResponse =
            serde_json::from_slice(&response_bytes).map_err(Self::protocol_error)?;
        if !response.validate_version() || response.request_id() != request_id {
            return Err(DruidError::DriverError(format!(
                "JDBC Agent response correlation failed: requestId={} expected={request_id}",
                response.request_id()
            )));
        }
        if let Some(error) = response.take_error() {
            return Err(error.into_druid_error());
        }
        Ok(response.take_result().unwrap_or(JsonValue::Null))
    }

    async fn read_json_line(&mut self) -> Result<JsonValue, DruidError> {
        let bytes = self.read_line().await?;
        serde_json::from_slice(&bytes).map_err(Self::protocol_error)
    }

    async fn read_line(&mut self) -> Result<Vec<u8>, DruidError> {
        let mut bytes = Vec::new();
        let length = self
            .stdout
            .read_until(b'\n', &mut bytes)
            .await
            .map_err(Self::io_error)?;
        if length == 0 {
            return Err(DruidError::DriverError(
                "JDBC Agent transport reached EOF".to_owned(),
            ));
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        self.validate_frame_size(bytes.len())?;
        Ok(bytes)
    }

    fn validate_frame_size(&self, length: usize) -> Result<(), DruidError> {
        if length > self.frame_limit {
            return Err(DruidError::DriverError(format!(
                "JDBC Agent frame size {length} exceeds configured limit {}",
                self.frame_limit
            )));
        }
        Ok(())
    }

    fn io_error(error: std::io::Error) -> DruidError {
        DruidError::DriverError(format!("JDBC Agent transport error: {error}"))
    }

    fn protocol_error(error: serde_json::Error) -> DruidError {
        DruidError::DriverError(format!("JDBC Agent protocol error: {error}"))
    }
}

impl Drop for AgentRuntime {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}
