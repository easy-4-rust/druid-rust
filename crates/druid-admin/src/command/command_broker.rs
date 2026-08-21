use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use druid_metrics::command_payload::canonical_command_payload;
use druid_metrics::protocol::{
    command, server_frame, Command, CommandAck, FrameHeader, RequestFullSnapshotCmd, ResetStatsCmd,
    ServerFrame,
};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tonic::Status;

use super::command_broker_inner::{CommandBrokerInner, CommandKey};

/// Admin HTTP 与真实 gRPC stream 共享的有界命令 broker。
///
/// Rust-only 管理面对象；命令以 stable stream selector、command ID 与 canonical
/// payload 登记，结果在同进程断线/重连期间保持可重放。
#[derive(Clone)]
pub struct CommandBroker {
    inner: Arc<Mutex<CommandBrokerInner>>,
    capacity: usize,
    result_timeout: Duration,
}

impl CommandBroker {
    /// 创建命令与结果日志容量均受限的 broker。
    #[must_use]
    pub fn new(capacity: usize, result_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(CommandBrokerInner::new())),
            capacity: capacity.max(1),
            result_timeout: result_timeout.max(Duration::from_millis(1)),
        }
    }

    /// 把 ResetStats 投递给当前 live stream 并等待 durable result。
    pub async fn issue_reset(
        &self,
        service_name: &str,
        instance_id: &str,
        boot_id: &str,
        command_id: &str,
        target_datasource_ids: Vec<u64>,
    ) -> Result<CommandAck, String> {
        self.issue(
            service_name,
            instance_id,
            boot_id,
            command_id,
            command::Payload::ResetStats(ResetStatsCmd {
                target_datasource_ids,
            }),
        )
        .await
    }

    /// 把 RequestFullSnapshot 投递给当前 live stream 并等待 durable result。
    pub async fn issue_full_snapshot(
        &self,
        service_name: &str,
        instance_id: &str,
        boot_id: &str,
        command_id: &str,
    ) -> Result<CommandAck, String> {
        self.issue(
            service_name,
            instance_id,
            boot_id,
            command_id,
            command::Payload::RequestFullSnapshot(RequestFullSnapshotCmd {}),
        )
        .await
    }

    /// 注册/替换一个 live stream，并立即重投该 identity 的全部未完成命令。
    pub(crate) fn attach_stream(
        &self,
        header: &FrameHeader,
        sender: mpsc::Sender<Result<ServerFrame, Status>>,
        cancellation: CancellationToken,
    ) -> Result<(), String> {
        let selector = stream_selector(header);
        let pending = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !inner.streams.contains_key(&selector) && inner.streams.len() >= self.capacity {
                return Err(format!(
                    "live stream broker full: capacity {}",
                    self.capacity
                ));
            }
            inner.streams.insert(
                selector.clone(),
                (header.clone(), sender.clone(), cancellation),
            );
            inner
                .order
                .iter()
                .filter_map(|key| {
                    (key.0 == selector)
                        .then(|| {
                            inner
                                .commands
                                .get(key)
                                .map(|record| (key.1.clone(), record))
                        })
                        .flatten()
                })
                .filter(|(_, record)| record.2.is_none())
                .map(|(command_id, record)| command_frame(header, command_id, record.1.clone()))
                .collect::<Vec<_>>()
        };
        for frame in pending {
            sender.try_send(Ok(frame)).map_err(|_| {
                format!(
                    "live stream command queue full: capacity {}",
                    sender.max_capacity()
                )
            })?;
        }
        Ok(())
    }

    /// 只在 sender 仍是当前连接时注销 live stream。
    pub(crate) fn detach_stream(
        &self,
        header: &FrameHeader,
        sender: &mpsc::Sender<Result<ServerFrame, Status>>,
    ) {
        let selector = stream_selector(header);
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if inner
            .streams
            .get(&selector)
            .is_some_and(|(_, current, _)| current.same_channel(sender))
        {
            inner.streams.remove(&selector);
        }
    }

    /// 取消一个当前 live stream，使客户端在同一 server 上走真实 reconnect。
    ///
    /// 命令 journal 与稳定结果不会被删除。
    pub fn cancel_stream(&self, service_name: &str, instance_id: &str, boot_id: &str) -> bool {
        let selector = stream_selector_parts(service_name, instance_id, boot_id);
        let stream = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .streams
            .remove(&selector);
        if let Some((_, _, cancellation)) = stream {
            cancellation.cancel();
            true
        } else {
            false
        }
    }

    /// 记录客户端 CommandAck，唤醒 HTTP waiter，并保留稳定结果用于 replay。
    pub(crate) fn record_result(
        &self,
        header: &FrameHeader,
        acknowledgement: &CommandAck,
    ) -> Result<(), String> {
        let key = (stream_selector(header), acknowledgement.command_id.clone());
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let selector = stream_selector(header);
        let current_header = inner
            .streams
            .get(&selector)
            .map(|(current_header, _, _)| current_header)
            .ok_or_else(|| "command acknowledgement stream is no longer live".to_owned())?;
        if current_header.stream_epoch != header.stream_epoch
            || current_header.protocol_version != header.protocol_version
        {
            return Err("command acknowledgement came from a stale stream epoch".to_owned());
        }
        let record = inner
            .commands
            .get_mut(&key)
            .ok_or_else(|| "command acknowledgement has no broker journal entry".to_owned())?;
        record.2 = Some(acknowledgement.clone());
        record.3.send_replace(Some(acknowledgement.clone()));
        Ok(())
    }

    async fn issue(
        &self,
        service_name: &str,
        instance_id: &str,
        boot_id: &str,
        command_id: &str,
        payload: command::Payload,
    ) -> Result<CommandAck, String> {
        if command_id.trim().is_empty() {
            return Err("command ID must be non-empty".to_owned());
        }
        let selector = stream_selector_parts(service_name, instance_id, boot_id);
        let key: CommandKey = (selector.clone(), command_id.to_owned());
        let canonical = canonical_command_payload(&Some(payload.clone())).map_err(str::to_owned)?;
        let (sender, frame, mut result_rx, inserted) = {
            let mut inner = self
                .inner
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(record) = inner.commands.get(&key) {
                if record.0 != canonical {
                    return Err("command ID has a conflicting canonical payload".to_owned());
                }
                if let Some(result) = record.2.clone() {
                    return Ok(result);
                }
                let (header, sender, _) =
                    inner.streams.get(&selector).cloned().ok_or_else(|| {
                        "no live stream matches the requested identity".to_owned()
                    })?;
                (
                    sender,
                    command_frame(&header, command_id.to_owned(), record.1.clone()),
                    record.3.subscribe(),
                    false,
                )
            } else {
                let (header, sender, _) =
                    inner.streams.get(&selector).cloned().ok_or_else(|| {
                        "no live stream matches the requested identity".to_owned()
                    })?;
                if !inner.make_room(self.capacity) {
                    return Err(format!("command broker full: capacity {}", self.capacity));
                }
                let (result_tx, result_rx) = watch::channel(None);
                inner
                    .commands
                    .insert(key.clone(), (canonical, payload.clone(), None, result_tx));
                inner.order.push_back(key.clone());
                (
                    sender,
                    command_frame(&header, command_id.to_owned(), payload),
                    result_rx,
                    true,
                )
            }
        };

        if sender.try_send(Ok(frame)).is_err() {
            if inserted {
                let mut inner = self
                    .inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                inner.commands.remove(&key);
                inner.order.retain(|existing| existing != &key);
            }
            return Err(format!(
                "live stream command queue full: capacity {}",
                sender.max_capacity()
            ));
        }
        // 投递完成后不持有 stream sender；否则等待结果的 HTTP future 会阻止
        // response channel 在连接轮换时关闭，客户端无法观察 EOF 并重连。
        drop(sender);

        tokio::time::timeout(self.result_timeout, async {
            loop {
                if let Some(result) = result_rx.borrow().clone() {
                    return Ok(result);
                }
                result_rx
                    .changed()
                    .await
                    .map_err(|_| "command result channel closed".to_owned())?;
            }
        })
        .await
        .map_err(|_| "command result deadline exceeded".to_owned())?
    }
}

impl fmt::Debug for CommandBroker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        formatter
            .debug_struct("CommandBroker")
            .field("capacity", &self.capacity)
            .field("result_timeout", &self.result_timeout)
            .field("live_streams", &inner.streams.len())
            .field("journal_entries", &inner.commands.len())
            .finish()
    }
}

fn command_frame(
    header: &FrameHeader,
    command_id: impl Into<String>,
    payload: command::Payload,
) -> ServerFrame {
    let mut header = header.clone();
    header.sequence = 0;
    header.emitted_at_unix_ms = now_unix_ms();
    ServerFrame {
        payload: Some(server_frame::Payload::Command(Command {
            header: Some(header),
            command_id: command_id.into(),
            payload: Some(payload),
        })),
    }
}

fn stream_selector(header: &FrameHeader) -> String {
    stream_selector_parts(&header.service_name, &header.instance_id, &header.boot_id)
}

fn stream_selector_parts(service_name: &str, instance_id: &str, boot_id: &str) -> String {
    format!(
        "{}:{service_name}{}:{instance_id}{}:{boot_id}",
        service_name.len(),
        instance_id.len(),
        boot_id.len()
    )
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
