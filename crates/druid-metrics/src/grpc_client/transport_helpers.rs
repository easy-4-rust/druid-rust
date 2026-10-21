use super::*;

fn collect_successor_command(
    exit: &mut PushWorkerExit,
    frame: ServerFrame,
    capacity: usize,
    progress: &PushProgress,
) -> Result<(), MetricsError> {
    let Some(server_frame::Payload::Command(command)) = frame.payload.as_ref() else {
        return Ok(());
    };
    let key = CommandWorkKey::from_command(command).map_err(other_error)?;
    if let Some(existing_command) = exit.successor_commands.iter().find_map(|existing| {
        let Some(server_frame::Payload::Command(existing_command)) = existing.payload.as_ref()
        else {
            return None;
        };
        (existing_command.command_id == command.command_id).then_some(existing_command)
    }) {
        if existing_command.payload == command.payload {
            progress.drop_transport_command(&key).map_err(other_error)?;
            return Ok(());
        }
        progress.drop_transport_command(&key).map_err(other_error)?;
        return Err(MetricsError::ChannelSend(
            "successor command ID has a conflicting payload".to_owned(),
        ));
    }
    if exit.successor_commands.len() >= capacity {
        progress.drop_transport_command(&key).map_err(other_error)?;
        return Err(MetricsError::ChannelSend(format!(
            "successor command queue full: capacity {capacity}"
        )));
    }
    exit.successor_commands.push(frame);
    Ok(())
}

async fn open_transport(
    channel: Channel,
    config: &GrpcClientConfig,
    stream_epoch: u64,
) -> Result<ConnectedTransport, MetricsError> {
    let mut client = MetricsIngestClient::new(channel);
    let (outbound_tx, outbound_rx) = mpsc::channel(config.ack_window_capacity);
    let mut request = tonic::Request::new(ReceiverStream::new(outbound_rx));
    if let Some(authorization) = &config.authorization {
        authorization.apply(&mut request);
    }
    let response = client.connect(request).await.map_err(other_error)?;
    let mut inbound = response.into_inner();
    let hello_header = make_header(config, stream_epoch, 0);
    outbound_tx
        .send(ClientFrame {
            payload: Some(client_frame::Payload::Hello(ClientHello {
                header: Some(hello_header.clone()),
                supported_protocol_version: 1,
                full_snapshot_requested: false,
            })),
        })
        .await
        .map_err(|_| MetricsError::ChannelSend("gRPC hello transport closed".to_owned()))?;
    let hello = inbound
        .message()
        .await
        .map_err(other_error)?
        .ok_or_else(|| MetricsError::ChannelSend("gRPC hello response stream closed".to_owned()))?;
    let require_full_snapshot = match hello.payload {
        Some(server_frame::Payload::HelloAck(ack)) => {
            let response_header = ack.header.as_ref().ok_or_else(|| {
                MetricsError::ChannelSend("HelloAck is missing its header".to_owned())
            })?;
            if ack.server_protocol_version != 1
                || response_header.protocol_version != 1
                || !same_stream_identity(&hello_header, response_header)
            {
                return Err(MetricsError::ChannelSend(
                    "HelloAck stream identity or protocol version does not match ClientHello"
                        .to_owned(),
                ));
            }
            ack.require_full_snapshot
        }
        other => {
            return Err(MetricsError::ChannelSend(format!(
                "expected HelloAck, received {other:?}"
            )));
        }
    };
    Ok(ConnectedTransport {
        outbound_tx,
        inbound,
        require_full_snapshot,
    })
}

async fn connect_transport(
    config: &GrpcClientConfig,
    stream_epoch: u64,
    cancellation: Option<&CancellationToken>,
    unflushed: usize,
) -> Result<ConnectedTransport, MetricsError> {
    let setup = async {
        let channel = config
            .endpoint
            .clone()
            .connect()
            .await
            .map_err(other_error)?;
        open_transport(channel, config, stream_epoch).await
    };
    let bounded = async {
        tokio::time::timeout(config.shutdown_timeout, setup)
            .await
            .map_err(|_| MetricsError::GrpcTransportSetupTimeout {
                timeout_millis: config.shutdown_timeout.as_millis() as u64,
            })?
    };
    if let Some(cancellation) = cancellation {
        tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(MetricsError::ShutdownTimeout { unflushed }),
            result = bounded => result,
        }
    } else {
        bounded.await
    }
}

struct AbortOnDrop(Option<JoinHandle<Result<Option<ServerFrame>, MetricsError>>>);

impl AbortOnDrop {
    fn new(handle: JoinHandle<Result<Option<ServerFrame>, MetricsError>>) -> Self {
        Self(Some(handle))
    }

    fn take(&mut self) -> Option<JoinHandle<Result<Option<ServerFrame>, MetricsError>>> {
        self.0.take()
    }

    fn is_finished(&self) -> bool {
        self.0.as_ref().is_none_or(JoinHandle::is_finished)
    }
}

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            handle.abort();
        }
    }
}

async fn join_reader_and_collect_successor_commands(
    reader_task: &mut AbortOnDrop,
    reader_cancellation: &CancellationToken,
    exit: &mut PushWorkerExit,
    progress: &PushProgress,
    timeout: Duration,
    capacity: usize,
) -> Result<Result<(), MetricsError>, MetricsError> {
    let Some(mut reader_handle) = reader_task.take() else {
        while let Ok(frame) = exit.transport_rx.try_recv() {
            collect_successor_command(exit, frame, capacity, progress)?;
        }
        return Ok(Ok(()));
    };

    let deadline = tokio::time::Instant::now() + timeout;
    let mut receiver_open = true;
    loop {
        tokio::select! {
            biased;
            frame = exit.transport_rx.recv(), if receiver_open => {
                match frame {
                    Some(frame) => collect_successor_command(exit, frame, capacity, progress)?,
                    None => receiver_open = false,
                }
            }
            result = &mut reader_handle => {
                let reader_result = flatten_reader_join(result);
                if let Ok(Some(frame)) = reader_result.as_ref() {
                    collect_successor_command(exit, frame.clone(), capacity, progress)?;
                }
                while let Ok(frame) = exit.transport_rx.try_recv() {
                    collect_successor_command(exit, frame, capacity, progress)?;
                }
                return Ok(reader_result.map(|_| ()));
            }
            () = tokio::time::sleep_until(deadline) => {
                reader_cancellation.cancel();
                let reader_result = flatten_reader_join(reader_handle.await);
                if let Ok(Some(frame)) = reader_result.as_ref() {
                    collect_successor_command(exit, frame.clone(), capacity, progress)?;
                }
                while let Ok(frame) = exit.transport_rx.try_recv() {
                    collect_successor_command(exit, frame, capacity, progress)?;
                }
                return Ok(Err(MetricsError::ShutdownTimeout { unflushed: 0 }));
            }
        }
    }
}

fn flatten_reader_join(
    result: Result<Result<Option<ServerFrame>, MetricsError>, tokio::task::JoinError>,
) -> Result<Option<ServerFrame>, MetricsError> {
    match result {
        Ok(result) => result,
        Err(error) => Err(MetricsError::Join(error)),
    }
}

async fn wait_for_reconnect(
    backoff: Duration,
    cancellation: &CancellationToken,
) -> Result<(), MetricsError> {
    tokio::select! {
        () = cancellation.cancelled() => Err(MetricsError::ShutdownTimeout { unflushed: 1 }),
        () = tokio::time::sleep(jittered(backoff)) => Ok(()),
    }
}

fn validate_config(config: &GrpcClientConfig) -> Result<(), MetricsError> {
    if config.ack_window_capacity == 0 {
        return Err(MetricsError::ChannelSend(
            "gRPC ACK window capacity must be greater than zero".to_owned(),
        ));
    }
    if config.shutdown_timeout.is_zero() {
        return Err(MetricsError::ChannelSend(
            "gRPC shutdown timeout must be non-zero".to_owned(),
        ));
    }
    if config.service_name.trim().is_empty()
        || config.instance_id.trim().is_empty()
        || config.boot_id.trim().is_empty()
    {
        return Err(MetricsError::ChannelSend(
            "gRPC stream identity fields must be non-empty".to_owned(),
        ));
    }
    crate::ServiceIdentity::try_new(&config.service_name, &config.instance_id, &config.boot_id)
        .map_err(|error| MetricsError::ChannelSend(error.to_owned()))?;
    let uri = config.endpoint.uri();
    let host = uri
        .host()
        .ok_or_else(|| MetricsError::ChannelSend("gRPC endpoint must contain a host".to_owned()))?;
    let normalized_host = host.trim_matches(['[', ']']);
    let is_loopback = normalized_host.eq_ignore_ascii_case("localhost")
        || normalized_host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !is_loopback && (uri.scheme_str() != Some("https") || config.authorization.is_none()) {
        return Err(MetricsError::ChannelSend(
            "remote gRPC endpoints require HTTPS and authentication".to_owned(),
        ));
    }
    Ok(())
}

fn flatten_join(
    result: Result<Result<(), MetricsError>, tokio::task::JoinError>,
) -> Result<(), MetricsError> {
    match result {
        Ok(result) => result,
        Err(error) => Err(MetricsError::Join(error)),
    }
}

fn make_header(config: &GrpcClientConfig, stream_epoch: u64, sequence: u64) -> FrameHeader {
    FrameHeader {
        protocol_version: 1,
        service_name: config.service_name.clone(),
        instance_id: config.instance_id.clone(),
        boot_id: config.boot_id.clone(),
        stream_epoch,
        sequence,
        emitted_at_unix_ms: now_unix_ms(),
    }
}

fn same_stream_identity(expected: &FrameHeader, actual: &FrameHeader) -> bool {
    expected.service_name == actual.service_name
        && expected.instance_id == actual.instance_id
        && expected.boot_id == actual.boot_id
        && expected.stream_epoch == actual.stream_epoch
}

fn validate_server_frame(
    frame: &ServerFrame,
    config: &GrpcClientConfig,
    stream_epoch: u64,
) -> Result<(), MetricsError> {
    let payload = frame
        .payload
        .as_ref()
        .ok_or_else(|| MetricsError::ChannelSend("server frame payload is missing".to_owned()))?;
    let header = match payload {
        server_frame::Payload::HelloAck(value) => {
            if value.server_protocol_version != 1 {
                return Err(MetricsError::ChannelSend(format!(
                    "unsupported server protocol version {}",
                    value.server_protocol_version
                )));
            }
            value.header.as_ref()
        }
        server_frame::Payload::BatchAck(value) => value.header.as_ref(),
        server_frame::Payload::ResyncRequired(value) => value.header.as_ref(),
        server_frame::Payload::Command(value) => value.header.as_ref(),
        server_frame::Payload::Error(value) => value.header.as_ref(),
    }
    .ok_or_else(|| MetricsError::ChannelSend("server frame header is missing".to_owned()))?;
    let expected = make_header(config, stream_epoch, 0);
    if header.protocol_version != 1 || !same_stream_identity(&expected, header) {
        return Err(MetricsError::ChannelSend(
            "server frame protocol version or stream identity does not match the client stream"
                .to_owned(),
        ));
    }
    Ok(())
}

fn next_stream_epoch() -> u64 {
    static NEXT_EPOCH: AtomicU64 = AtomicU64::new(1);
    now_unix_ms()
        .saturating_mul(1_000)
        .saturating_add(NEXT_EPOCH.fetch_add(1, Ordering::Relaxed))
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn next_backoff(current: Duration) -> Duration {
    current.saturating_mul(2).min(MAX_BACKOFF)
}

fn jittered(base: Duration) -> Duration {
    static JITTER: AtomicU64 = AtomicU64::new(0x9e37_79b9_7f4a_7c15);
    let mut value = JITTER.fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed);
    value ^= value >> 12;
    value ^= value << 25;
    value ^= value >> 27;
    let percent = 100_u128 + u128::from(value % 26);
    let nanos = base.as_nanos().saturating_mul(percent) / 100;
    Duration::from_nanos(nanos.min(MAX_BACKOFF.as_nanos()) as u64)
}

fn other_error(error: impl std::error::Error + Send + Sync + 'static) -> MetricsError {
    MetricsError::Other(Box::new(error))
}
