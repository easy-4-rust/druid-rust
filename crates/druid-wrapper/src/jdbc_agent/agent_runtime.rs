use super::{AgentRequest, AgentResponse, JdbcAgentOptions, JdbcAgentRuntimeMetrics};
use druid_core::core::DruidError;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::fs::File;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};

type PendingRequests = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<JsonValue, DruidError>>>>>;

struct OutboundFrame {
    request_id: u64,
    bytes: Vec<u8>,
}

/// 已发送、等待按 JSON-RPC ID 关联响应的 Agent 请求。
pub(crate) struct AgentPendingRequest {
    request_id: u64,
    receiver: oneshot::Receiver<Result<JsonValue, DruidError>>,
    pending: PendingRequests,
}

impl AgentPendingRequest {
    /// 返回请求 ID，供超时后的 `cancel` 定位原 JDBC Statement。
    pub(crate) const fn request_id(&self) -> u64 {
        self.request_id
    }

    /// 等待 Agent 响应；传输任务终止时返回连接已丢弃。
    pub(crate) async fn wait(mut self) -> Result<JsonValue, DruidError> {
        let result = (&mut self.receiver)
            .await
            .unwrap_or(Err(DruidError::ConnectionDiscarded));
        Self::remove_pending(&self.pending, self.request_id);
        result
    }

    fn remove_pending(pending: &PendingRequests, request_id: u64) {
        pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&request_id);
    }
}

impl Drop for AgentPendingRequest {
    fn drop(&mut self) {
        Self::remove_pending(&self.pending, self.request_id);
    }
}

/// 一个可承载多个 JDBC session 的 Agent 子进程。
///
/// 独立写任务保证 NDJSON 帧不交错，独立读任务按 request ID 将乱序响应投递给
/// 等待者；数据库连接隔离由协议中的 `sessionId` 保证。对应 Java:
/// `AgentServer` JSON-RPC 循环。
pub(crate) struct AgentRuntime {
    child: Mutex<Child>,
    outbound: mpsc::UnboundedSender<OutboundFrame>,
    pending: PendingRequests,
    next_request_id: AtomicU64,
    frame_limit: usize,
    healthy: Arc<AtomicBool>,
    expected_shutdown: Arc<AtomicBool>,
    counted_process: bool,
    _artifact_leases: Vec<Arc<File>>,
}

impl AgentRuntime {
    /// 启动 Agent，校验 ready 通知并完成版本/能力握手。
    pub(crate) async fn start(
        options: &JdbcAgentOptions,
        healthy: Arc<AtomicBool>,
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
        let mut stdout = BufReader::new(stdout);
        let ready = Self::read_json_line(&mut stdout, options.frame_limit()).await?;
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

        let pending = Arc::new(Mutex::new(HashMap::new()));
        let expected_shutdown = Arc::new(AtomicBool::new(false));
        let (outbound, outbound_receiver) = mpsc::unbounded_channel();
        tokio::spawn(Self::write_loop(
            stdin,
            outbound_receiver,
            Arc::clone(&pending),
            Arc::clone(&healthy),
            Arc::clone(&expected_shutdown),
        ));
        tokio::spawn(Self::read_loop(
            stdout,
            options.frame_limit(),
            Arc::clone(&pending),
            Arc::clone(&healthy),
            Arc::clone(&expected_shutdown),
        ));

        let mut runtime = Self {
            child: Mutex::new(child),
            outbound,
            pending,
            next_request_id: AtomicU64::new(1),
            frame_limit: options.frame_limit(),
            healthy,
            expected_shutdown,
            counted_process: false,
            _artifact_leases: options.artifact_leases(),
        };
        let required_capabilities = [
            "multi-session",
            "structured-errors",
            "tagged-values",
            "concurrent-requests",
            "cursor-paging",
            "cancel",
            "remote-prepare",
            "native-prepared-batch",
        ];
        let handshake = runtime
            .request(
                "handshake",
                json!({
                    "protocolVersion": 1,
                    "client": "druid-rust",
                    "driverArtifactVersion": options.artifact_version(),
                    "capabilities": required_capabilities,
                    "contractFaultInjection": options.contract_fault_injection_enabled()
                }),
            )
            .await?;
        Self::validate_handshake(
            &handshake,
            options.artifact_version(),
            &required_capabilities,
        )?;
        JdbcAgentRuntimeMetrics::process_started();
        runtime.counted_process = true;
        Ok(runtime)
    }

    fn validate_handshake(
        handshake: &JsonValue,
        artifact_version: &str,
        required_capabilities: &[&str],
    ) -> Result<(), DruidError> {
        let protocol_version = handshake.get("protocolVersion").and_then(JsonValue::as_u64);
        let agent_version = handshake
            .get("agentVersion")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty());
        let negotiated_artifact = handshake
            .get("driverArtifactVersion")
            .and_then(JsonValue::as_str);
        let capabilities = handshake.get("capabilities").and_then(JsonValue::as_array);
        if protocol_version != Some(1)
            || agent_version.is_none()
            || negotiated_artifact != Some(artifact_version)
            || capabilities.is_none()
        {
            return Err(Self::protocol_message(format!(
                "invalid handshake response: {handshake}"
            )));
        }
        let capabilities = capabilities.expect("capabilities were checked above");
        for required in required_capabilities {
            if !capabilities
                .iter()
                .any(|value| value.as_str() == Some(required))
            {
                return Err(Self::protocol_message(format!(
                    "handshake lacks required capability '{required}'"
                )));
            }
        }
        Ok(())
    }

    /// 创建并发送请求，返回可独立等待的关联句柄。
    pub(crate) fn begin_request(
        &self,
        method: &str,
        params: JsonValue,
    ) -> Result<AgentPendingRequest, DruidError> {
        if !self.healthy.load(Ordering::Acquire) {
            return Err(DruidError::ConnectionDiscarded);
        }
        if method == "shutdown" {
            self.expected_shutdown.store(true, Ordering::Release);
        }
        let request_id = self.next_request_id.fetch_add(1, Ordering::AcqRel).max(1);
        let request = AgentRequest::new(request_id, method, params);
        let mut bytes = serde_json::to_vec(&request).map_err(Self::protocol_error)?;
        self.validate_frame_size(bytes.len())?;
        bytes.push(b'\n');

        let (sender, receiver) = oneshot::channel();
        self.pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(request_id, sender);
        if self
            .outbound
            .send(OutboundFrame { request_id, bytes })
            .is_err()
        {
            AgentPendingRequest::remove_pending(&self.pending, request_id);
            self.healthy.store(false, Ordering::Release);
            return Err(DruidError::ConnectionDiscarded);
        }
        Ok(AgentPendingRequest {
            request_id,
            receiver,
            pending: Arc::clone(&self.pending),
        })
    }

    /// 发送一个 JSON-RPC 请求并按 ID 关联响应。
    pub(crate) async fn request(
        &self,
        method: &str,
        params: JsonValue,
    ) -> Result<JsonValue, DruidError> {
        self.begin_request(method, params)?.wait().await
    }

    async fn write_loop(
        mut stdin: ChildStdin,
        mut outbound: mpsc::UnboundedReceiver<OutboundFrame>,
        pending: PendingRequests,
        healthy: Arc<AtomicBool>,
        expected_shutdown: Arc<AtomicBool>,
    ) {
        while let Some(frame) = outbound.recv().await {
            if let Err(error) = stdin.write_all(&frame.bytes).await {
                Self::fail_transport(
                    &pending,
                    &healthy,
                    &expected_shutdown,
                    format!("JDBC Agent transport write error: {error}"),
                );
                return;
            }
            if let Err(error) = stdin.flush().await {
                Self::fail_transport(
                    &pending,
                    &healthy,
                    &expected_shutdown,
                    format!("JDBC Agent transport flush error: {error}"),
                );
                return;
            }
            if !healthy.load(Ordering::Acquire) {
                AgentPendingRequest::remove_pending(&pending, frame.request_id);
                return;
            }
        }
    }

    async fn read_loop(
        mut stdout: BufReader<ChildStdout>,
        frame_limit: usize,
        pending: PendingRequests,
        healthy: Arc<AtomicBool>,
        expected_shutdown: Arc<AtomicBool>,
    ) {
        loop {
            let response_bytes = match Self::read_line(&mut stdout, frame_limit).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    if error.to_string().contains("frame size") {
                        JdbcAgentRuntimeMetrics::protocol_error();
                    }
                    Self::fail_transport(&pending, &healthy, &expected_shutdown, error.to_string());
                    return;
                }
            };
            let mut response: AgentResponse = match serde_json::from_slice(&response_bytes) {
                Ok(response) => response,
                Err(error) => {
                    Self::fail_transport(
                        &pending,
                        &healthy,
                        &expected_shutdown,
                        format!("JDBC Agent protocol error: {error}"),
                    );
                    JdbcAgentRuntimeMetrics::protocol_error();
                    return;
                }
            };
            if !response.validate_version() {
                Self::fail_transport(
                    &pending,
                    &healthy,
                    &expected_shutdown,
                    "JDBC Agent response has an invalid JSON-RPC version".to_owned(),
                );
                JdbcAgentRuntimeMetrics::protocol_error();
                return;
            }
            let request_id = response.request_id();
            let sender = pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(&request_id);
            // 超时请求的迟到响应没有等待者，安全丢弃即可。
            if let Some(sender) = sender {
                let result = if let Some(error) = response.take_error() {
                    Err(error.into_druid_error())
                } else {
                    Ok(response.take_result().unwrap_or(JsonValue::Null))
                };
                let _ = sender.send(result);
            }
        }
    }

    async fn read_json_line(
        stdout: &mut BufReader<ChildStdout>,
        frame_limit: usize,
    ) -> Result<JsonValue, DruidError> {
        let bytes = Self::read_line(stdout, frame_limit).await?;
        serde_json::from_slice(&bytes).map_err(Self::protocol_error)
    }

    async fn read_line(
        stdout: &mut BufReader<ChildStdout>,
        frame_limit: usize,
    ) -> Result<Vec<u8>, DruidError> {
        let mut bytes = Vec::new();
        let length = stdout
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
        if bytes.len() > frame_limit {
            return Err(DruidError::DriverError(format!(
                "JDBC Agent frame size {} exceeds configured limit {frame_limit}",
                bytes.len()
            )));
        }
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

    fn fail_transport(
        pending: &PendingRequests,
        healthy: &AtomicBool,
        expected_shutdown: &AtomicBool,
        message: String,
    ) {
        if healthy.swap(false, Ordering::AcqRel) && !expected_shutdown.load(Ordering::Acquire) {
            JdbcAgentRuntimeMetrics::process_crashed();
        }
        let senders = pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain()
            .map(|(_, sender)| sender)
            .collect::<Vec<_>>();
        for sender in senders {
            let _ = sender.send(Err(DruidError::DriverError(message.clone())));
        }
    }

    fn io_error(error: std::io::Error) -> DruidError {
        DruidError::DriverError(format!("JDBC Agent transport error: {error}"))
    }

    fn protocol_error(error: serde_json::Error) -> DruidError {
        DruidError::DriverError(format!("JDBC Agent protocol error: {error}"))
    }

    fn protocol_message(message: impl std::fmt::Display) -> DruidError {
        DruidError::DriverError(format!("JDBC Agent protocol error: {message}"))
    }
}

impl Drop for AgentRuntime {
    fn drop(&mut self) {
        self.expected_shutdown.store(true, Ordering::Release);
        self.healthy.store(false, Ordering::Release);
        if self.counted_process {
            JdbcAgentRuntimeMetrics::process_stopped();
        }
        if let Ok(mut child) = self.child.lock() {
            let _ = child.start_kill();
        }
    }
}
