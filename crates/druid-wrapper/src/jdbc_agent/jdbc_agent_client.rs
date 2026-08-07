use super::{AgentRequest, AgentResponse, JdbcAgentOptions};
use druid::core::DruidError;
use serde_json::Value as JsonValue;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

/// 拥有单个 JDBC Agent 子进程和有界二进制帧通道的客户端。
pub(crate) struct JdbcAgentClient {
    child: Child,
    stdin: ChildStdin,
    stdout: ChildStdout,
    options: JdbcAgentOptions,
    next_request_id: u64,
    closed: bool,
}

impl JdbcAgentClient {
    /// 启动 Agent 并完成连接握手。
    pub(crate) async fn connect(
        options: JdbcAgentOptions,
        connect_payload: JsonValue,
    ) -> Result<Self, DruidError> {
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
        let mut client = Self {
            child,
            stdin,
            stdout,
            options,
            next_request_id: 1,
            closed: false,
        };
        client.request("connect", connect_payload).await?;
        Ok(client)
    }

    /// 执行一次请求并校验协议版本、关联 ID 与结构化错误。
    pub(crate) async fn request(
        &mut self,
        operation: &str,
        payload: JsonValue,
    ) -> Result<JsonValue, DruidError> {
        if self.closed {
            return Err(DruidError::ConnectionDiscarded);
        }
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let request = AgentRequest::new(request_id, operation, payload);
        let timeout = self.options.timeout();
        if let Ok(result) = tokio::time::timeout(timeout, self.exchange(request_id, &request)).await
        {
            if matches!(
                result,
                Err(DruidError::DriverError(_) | DruidError::ConnectionDiscarded)
            ) {
                self.discard();
            }
            result
        } else {
            self.discard();
            Err(DruidError::DriverError(format!(
                "JDBC Agent operation '{operation}' exceeded {timeout:?}"
            )))
        }
    }

    async fn exchange(
        &mut self,
        request_id: u64,
        request: &AgentRequest,
    ) -> Result<JsonValue, DruidError> {
        let bytes = serde_json::to_vec(request).map_err(|error| Self::protocol_error(&error))?;
        self.validate_frame_size(bytes.len())?;
        let length = u32::try_from(bytes.len()).map_err(|_| {
            DruidError::DriverError("JDBC Agent request frame exceeds u32 length".to_owned())
        })?;
        self.stdin
            .write_all(&length.to_be_bytes())
            .await
            .map_err(|error| Self::io_error(&error))?;
        self.stdin
            .write_all(&bytes)
            .await
            .map_err(|error| Self::io_error(&error))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| Self::io_error(&error))?;

        let mut length_bytes = [0_u8; 4];
        self.stdout
            .read_exact(&mut length_bytes)
            .await
            .map_err(|error| Self::io_error(&error))?;
        let response_length = u32::from_be_bytes(length_bytes) as usize;
        self.validate_frame_size(response_length)?;
        let mut response_bytes = vec![0_u8; response_length];
        self.stdout
            .read_exact(&mut response_bytes)
            .await
            .map_err(|error| Self::io_error(&error))?;
        let mut response: AgentResponse = serde_json::from_slice(&response_bytes)
            .map_err(|error| Self::protocol_error(&error))?;
        if response.protocol_version() != 1 || response.request_id() != request_id {
            return Err(DruidError::DriverError(format!(
                "JDBC Agent response correlation failed: protocol={}, requestId={} expected={request_id}",
                response.protocol_version(),
                response.request_id()
            )));
        }
        if response.success() {
            Ok(response.take_payload().unwrap_or(JsonValue::Null))
        } else {
            Err(response.take_error().map_or_else(
                || {
                    DruidError::DriverError(
                        "JDBC Agent returned failure without structured error".to_owned(),
                    )
                },
                super::AgentError::into_druid_error,
            ))
        }
    }

    /// 请求 Agent 关闭连接，并确保子进程退出。
    pub(crate) async fn close(&mut self) -> Result<(), DruidError> {
        if self.closed {
            return Ok(());
        }
        let result = self.request("close", JsonValue::Null).await.map(|_| ());
        self.closed = true;
        if tokio::time::timeout(self.options.timeout(), self.child.wait())
            .await
            .is_err()
        {
            let _ = self.child.kill().await;
        }
        result
    }

    fn validate_frame_size(&self, length: usize) -> Result<(), DruidError> {
        if length > self.options.frame_limit() {
            return Err(DruidError::DriverError(format!(
                "JDBC Agent frame size {length} exceeds configured limit {}",
                self.options.frame_limit()
            )));
        }
        Ok(())
    }

    fn io_error(error: &std::io::Error) -> DruidError {
        DruidError::DriverError(format!("JDBC Agent transport error: {error}"))
    }

    fn protocol_error(error: &serde_json::Error) -> DruidError {
        DruidError::DriverError(format!("JDBC Agent protocol error: {error}"))
    }

    pub(crate) const fn is_unusable(&self) -> bool {
        self.closed
    }

    fn discard(&mut self) {
        self.closed = true;
        let _ = self.child.start_kill();
    }
}

impl Drop for JdbcAgentClient {
    fn drop(&mut self) {
        if !self.closed {
            self.discard();
        }
    }
}
