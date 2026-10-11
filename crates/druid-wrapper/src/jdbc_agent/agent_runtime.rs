use super::{AgentRequest, AgentResponse, JdbcAgentOptions, JdbcAgentRuntimeMetrics};
use druid_rdbc::error::RdbcError;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::fs::File;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{mpsc, oneshot};

type PendingRequests = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<JsonValue, RdbcError>>>>>;
// 默认单帧上限 16 MiB；限制排队帧数，防止停滞的 Agent 使客户端无限缓存请求。
const OUTBOUND_QUEUE_CAPACITY: usize = 8;
// 多 session 共享同一进程；Agent 长时间不响应时仍需限制关联状态规模。
const MAX_PENDING_REQUESTS: usize = 64;

struct OutboundFrame {
    request_id: u64,
    bytes: Vec<u8>,
    shutdown: bool,
}

/// 已发送、等待按 JSON-RPC ID 关联响应的 Agent 请求。
pub(crate) struct AgentPendingRequest {
    request_id: u64,
    receiver: oneshot::Receiver<Result<JsonValue, RdbcError>>,
    pending: PendingRequests,
}

impl AgentPendingRequest {
    /// 返回请求 ID，供超时后的 `cancel` 定位原 JDBC Statement。
    pub(crate) const fn request_id(&self) -> u64 {
        self.request_id
    }

    /// 等待 Agent 响应；传输任务终止时返回连接已丢弃。
    pub(crate) async fn wait(mut self) -> Result<JsonValue, RdbcError> {
        let result = (&mut self.receiver)
            .await
            .unwrap_or(Err(RdbcError::ConnectionDiscarded));
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
    outbound: mpsc::Sender<OutboundFrame>,
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
    ) -> Result<Self, RdbcError> {
        let mut command = Command::new(options.program());
        command
            .args(options.arguments())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| {
            RdbcError::DriverError(format!("failed to spawn JDBC Agent: {error}"))
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| RdbcError::DriverError("JDBC Agent stdin was not piped".to_owned()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RdbcError::DriverError("JDBC Agent stdout was not piped".to_owned()))?;
        let mut stdout = BufReader::new(stdout);
        let ready = Self::read_json_line(&mut stdout, options.frame_limit()).await?;
        Self::validate_ready(&ready)?;

        let pending = Arc::new(Mutex::new(HashMap::new()));
        let expected_shutdown = Arc::new(AtomicBool::new(false));
        let (outbound, outbound_receiver) = mpsc::channel(OUTBOUND_QUEUE_CAPACITY);
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

    fn validate_ready(ready: &JsonValue) -> Result<(), RdbcError> {
        if ready.get("jsonrpc").and_then(JsonValue::as_str) != Some("2.0")
            || ready.get("method").and_then(JsonValue::as_str) != Some("ready")
            || ready
                .pointer("/params/protocolVersion")
                .and_then(JsonValue::as_u64)
                != Some(1)
        {
            return Err(RdbcError::DriverError(
                "JDBC Agent sent an invalid ready notification".to_owned(),
            ));
        }
        Ok(())
    }

    fn validate_handshake(
        handshake: &JsonValue,
        artifact_version: &str,
        required_capabilities: &[&str],
    ) -> Result<(), RdbcError> {
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
            return Err(Self::protocol_message("invalid handshake response"));
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
    ) -> Result<AgentPendingRequest, RdbcError> {
        if !self.healthy.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let request_id = self.next_request_id.fetch_add(1, Ordering::AcqRel).max(1);
        let request = AgentRequest::new(request_id, method, params);
        let mut bytes = serde_json::to_vec(&request).map_err(Self::protocol_error)?;
        self.validate_frame_size(bytes.len())?;
        bytes.push(b'\n');

        let (sender, receiver) = oneshot::channel();
        Self::register_pending(&self.pending, request_id, sender)?;
        if let Err(error) = Self::queue_frame(
            &self.outbound,
            &self.pending,
            OutboundFrame {
                request_id,
                bytes,
                shutdown: method == "shutdown",
            },
        ) {
            if matches!(error, RdbcError::ConnectionDiscarded) {
                self.healthy.store(false, Ordering::Release);
            }
            return Err(error);
        }
        Ok(AgentPendingRequest {
            request_id,
            receiver,
            pending: Arc::clone(&self.pending),
        })
    }

    fn register_pending(
        pending: &PendingRequests,
        request_id: u64,
        sender: oneshot::Sender<Result<JsonValue, RdbcError>>,
    ) -> Result<(), RdbcError> {
        let mut requests = pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if requests.len() >= MAX_PENDING_REQUESTS {
            return Err(RdbcError::DriverError(
                "JDBC Agent pending request limit reached".to_owned(),
            ));
        }
        if requests.contains_key(&request_id) {
            return Err(RdbcError::DriverError(
                "JDBC Agent request ID collision".to_owned(),
            ));
        }
        requests.insert(request_id, sender);
        Ok(())
    }

    fn queue_frame(
        outbound: &mpsc::Sender<OutboundFrame>,
        pending: &PendingRequests,
        frame: OutboundFrame,
    ) -> Result<(), RdbcError> {
        let request_id = frame.request_id;
        match outbound.try_send(frame) {
            Ok(()) => Ok(()),
            Err(error) => {
                AgentPendingRequest::remove_pending(pending, request_id);
                match error {
                    mpsc::error::TrySendError::Full(_) => Err(RdbcError::DriverError(
                        "JDBC Agent outbound request queue is full".to_owned(),
                    )),
                    mpsc::error::TrySendError::Closed(_) => Err(RdbcError::ConnectionDiscarded),
                }
            }
        }
    }

    /// 发送一个 JSON-RPC 请求并按 ID 关联响应。
    pub(crate) async fn request(
        &self,
        method: &str,
        params: JsonValue,
    ) -> Result<JsonValue, RdbcError> {
        self.begin_request(method, params)?.wait().await
    }

    async fn write_loop(
        mut stdin: ChildStdin,
        mut outbound: mpsc::Receiver<OutboundFrame>,
        pending: PendingRequests,
        healthy: Arc<AtomicBool>,
        expected_shutdown: Arc<AtomicBool>,
    ) {
        while let Some(frame) = outbound.recv().await {
            if frame.shutdown {
                expected_shutdown.store(true, Ordering::Release);
            }
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
    ) -> Result<JsonValue, RdbcError> {
        let bytes = Self::read_line(stdout, frame_limit).await?;
        serde_json::from_slice(&bytes).map_err(Self::protocol_error)
    }

    async fn read_line<R: AsyncBufRead + Unpin>(
        stdout: &mut R,
        frame_limit: usize,
    ) -> Result<Vec<u8>, RdbcError> {
        let mut bytes = Vec::new();
        loop {
            let available = stdout.fill_buf().await.map_err(Self::io_error)?;
            if available.is_empty() {
                if bytes.is_empty() {
                    return Err(RdbcError::DriverError(
                        "JDBC Agent transport reached EOF".to_owned(),
                    ));
                }
                break;
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let take = newline.map_or(available.len(), |position| position + 1);
            let observed = bytes.len().saturating_add(take);
            // CRLF 允许比正文上限多两个终止字节；未结束的帧最多暂存一个 CR。
            let allowed = frame_limit.saturating_add(if newline.is_some() { 2 } else { 1 });
            if observed > allowed
                || (newline.is_none() && observed > frame_limit && available[take - 1] != b'\r')
            {
                return Err(RdbcError::DriverError(format!(
                    "JDBC Agent frame size {observed} exceeds configured limit {frame_limit}"
                )));
            }
            bytes.extend_from_slice(&available[..take]);
            stdout.consume(take);
            if newline.is_some() {
                break;
            }
        }
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
        }
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        if bytes.len() > frame_limit {
            return Err(RdbcError::DriverError(format!(
                "JDBC Agent frame size {} exceeds configured limit {frame_limit}",
                bytes.len()
            )));
        }
        Ok(bytes)
    }

    fn validate_frame_size(&self, length: usize) -> Result<(), RdbcError> {
        if length > self.frame_limit {
            return Err(RdbcError::DriverError(format!(
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
            let _ = sender.send(Err(RdbcError::DriverError(message.clone())));
        }
    }

    fn io_error(error: std::io::Error) -> RdbcError {
        RdbcError::DriverError(format!("JDBC Agent transport error: {error}"))
    }

    fn protocol_error(error: serde_json::Error) -> RdbcError {
        RdbcError::DriverError(format!("JDBC Agent protocol error: {error}"))
    }

    fn protocol_message(message: impl std::fmt::Display) -> RdbcError {
        RdbcError::DriverError(format!("JDBC Agent protocol error: {message}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn pending_requests_have_a_hard_capacity() {
        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));
        for request_id in 1..=MAX_PENDING_REQUESTS as u64 {
            let (sender, _receiver) = oneshot::channel();
            AgentRuntime::register_pending(&pending, request_id, sender)
                .expect("request within capacity");
        }
        let (sender, _receiver) = oneshot::channel();
        let result =
            AgentRuntime::register_pending(&pending, MAX_PENDING_REQUESTS as u64 + 1, sender);
        assert!(
            matches!(result, Err(RdbcError::DriverError(message)) if message.contains("pending request limit"))
        );
        assert_eq!(
            pending.lock().expect("pending map").len(),
            MAX_PENDING_REQUESTS
        );
    }

    #[test]
    fn saturated_outbound_queue_rejects_and_cleans_pending_request() {
        let (outbound, _receiver) = mpsc::channel(1);
        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));
        outbound
            .try_send(OutboundFrame {
                request_id: 1,
                bytes: b"first\n".to_vec(),
                shutdown: false,
            })
            .expect("fill bounded outbound queue");
        let (sender, _response) = oneshot::channel();
        pending.lock().expect("pending map").insert(2, sender);

        let result = AgentRuntime::queue_frame(
            &outbound,
            &pending,
            OutboundFrame {
                request_id: 2,
                bytes: b"second\n".to_vec(),
                shutdown: false,
            },
        );
        assert!(
            matches!(result, Err(RdbcError::DriverError(message)) if message.contains("queue is full"))
        );
        assert!(!pending.lock().expect("pending map").contains_key(&2));
    }

    #[test]
    fn closed_outbound_queue_discards_and_cleans_pending_request() {
        let (outbound, receiver) = mpsc::channel(1);
        drop(receiver);
        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));
        let (sender, _response) = oneshot::channel();
        pending.lock().expect("pending map").insert(3, sender);

        let result = AgentRuntime::queue_frame(
            &outbound,
            &pending,
            OutboundFrame {
                request_id: 3,
                bytes: b"third\n".to_vec(),
                shutdown: false,
            },
        );
        assert!(matches!(result, Err(RdbcError::ConnectionDiscarded)));
        assert!(!pending.lock().expect("pending map").contains_key(&3));
    }

    #[tokio::test]
    async fn oversized_unterminated_frame_fails_before_eof() {
        let (mut writer, reader) = tokio::io::duplex(8192);
        writer
            .write_all(&vec![b'x'; 4096])
            .await
            .expect("write oversized frame without newline");
        let mut reader = BufReader::new(reader);
        let result = tokio::time::timeout(
            Duration::from_millis(200),
            AgentRuntime::read_line(&mut reader, 1024),
        )
        .await
        .expect("oversized frame must fail before EOF or newline");
        assert!(
            matches!(result, Err(RdbcError::DriverError(message)) if message.contains("frame size"))
        );
    }

    #[tokio::test]
    async fn frame_limit_preserves_crlf_and_eof_terminated_frames() {
        let (mut writer, reader) = tokio::io::duplex(16);
        writer
            .write_all(b"abc\r\n")
            .await
            .expect("write CRLF frame");
        let mut reader = BufReader::new(reader);
        assert_eq!(
            AgentRuntime::read_line(&mut reader, 3).await.unwrap(),
            b"abc"
        );

        let (mut writer, reader) = tokio::io::duplex(16);
        writer.write_all(b"abc").await.expect("write EOF frame");
        drop(writer);
        let mut reader = BufReader::new(reader);
        assert_eq!(
            AgentRuntime::read_line(&mut reader, 3).await.unwrap(),
            b"abc"
        );
    }

    #[test]
    fn invalid_handshake_does_not_echo_agent_payload() {
        let secret = "agent-secret-marker";
        let handshake = json!({"protocolVersion": 2, "password": secret});
        let error = AgentRuntime::validate_handshake(&handshake, "unmanaged", &[])
            .expect_err("invalid handshake must be rejected");
        assert!(!error.to_string().contains(secret));
    }

    #[test]
    fn invalid_ready_does_not_echo_agent_payload() {
        let secret = "agent-ready-secret-marker";
        let ready = json!({"jsonrpc": "2.0", "method": "wrong", "params": {"protocolVersion": 1, "password": secret}});
        let error =
            AgentRuntime::validate_ready(&ready).expect_err("invalid ready must be rejected");
        assert!(!error.to_string().contains(secret));
    }
}
