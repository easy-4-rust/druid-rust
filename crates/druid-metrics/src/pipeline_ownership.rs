use std::collections::{HashMap, VecDeque};

use parking_lot::Mutex;
use thiserror::Error;
use tokio::sync::Notify;

use crate::command_payload::canonical_command_payload;
use crate::protocol::{command, Command};
use crate::ServiceIdentity;

/// Pipeline ownership 状态机错误。
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum PipelineOwnershipError {
    /// 调用方执行了不符合当前 owner/stage 的转移或完成操作。
    #[error("{0}")]
    InvalidTransition(&'static str),
}

impl From<&'static str> for PipelineOwnershipError {
    fn from(message: &'static str) -> Self {
        Self::InvalidTransition(message)
    }
}

impl PipelineOwnershipError {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::InvalidTransition(message) => message,
        }
    }
}

/// 一个 batch 从 aggregator 到 ACK/丢弃结束的单调工作标识。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BatchWorkId(u64);

impl BatchWorkId {
    pub(crate) fn from_raw(value: u64) -> Self {
        Self(value)
    }

    pub(crate) fn into_raw(self) -> u64 {
        self.0
    }
}

/// 一条已验证命令在 transport/processing/durable reply 阶段的稳定键。
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CommandWorkKey {
    service_identity: ServiceIdentity,
    command_id: String,
    canonical_payload: Vec<u8>,
}

impl CommandWorkKey {
    pub(crate) fn from_parts(
        service_name: &str,
        instance_id: &str,
        boot_id: &str,
        command_id: &str,
        payload: &Option<command::Payload>,
    ) -> Result<Self, PipelineOwnershipError> {
        if command_id.trim().is_empty() {
            return Err("command ID must be non-empty".into());
        }
        Ok(Self {
            service_identity: ServiceIdentity::try_new(service_name, instance_id, boot_id)
                .map_err(PipelineOwnershipError::from)?,
            command_id: command_id.to_owned(),
            canonical_payload: canonical_command_payload(payload)
                .map_err(PipelineOwnershipError::from)?,
        })
    }

    pub(crate) fn from_command(command: &Command) -> Result<Self, PipelineOwnershipError> {
        let header = command
            .header
            .as_ref()
            .ok_or(PipelineOwnershipError::InvalidTransition(
                "command header is missing",
            ))?;
        Self::from_parts(
            &header.service_name,
            &header.instance_id,
            &header.boot_id,
            &command.command_id,
            &command.payload,
        )
    }

    #[cfg(test)]
    fn for_test(command_id: &str, payload: &[u8]) -> Self {
        Self {
            service_identity: ServiceIdentity::try_new("test", "instance", "boot")
                .expect("test identity is valid"),
            command_id: command_id.to_owned(),
            canonical_payload: payload.to_vec(),
        }
    }
}

/// 命令从 transport queue 转入 processing 后的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandTransition {
    /// 首次进入 processing，创建一个 durable owner。
    Started,
    /// 同 key 重投借用已有 durable owner，不创建第二个 owner。
    Coalesced,
}

#[derive(Debug, Default)]
struct CommandOwnership {
    queued: usize,
    processing: bool,
}

#[derive(Debug, Default)]
struct PipelineOwnershipState {
    aggregator_snapshots: usize,
    next_work_id: u64,
    exporter_batches: VecDeque<BatchWorkId>,
    client_queued_batches: VecDeque<BatchWorkId>,
    pending_frames: VecDeque<BatchWorkId>,
    commands: HashMap<CommandWorkKey, CommandOwnership>,
}

/// Runtime pipeline 各阶段互斥所有权的单一计数事实源。
///
/// Rust-only 并发对象；batch 使用单调 [`BatchWorkId`]，command 使用完整
/// [`CommandWorkKey`]。所有 handoff 都在一个锁内同时减旧 owner、加新 owner，
/// shutdown 因而不会观察到重叠或空窗；非法 double-finish 与跨 key 操作返回错误。
#[derive(Debug)]
pub(crate) struct PipelineOwnership {
    state: Mutex<PipelineOwnershipState>,
    changed: Notify,
}

impl PipelineOwnership {
    /// 创建全部阶段计数为零的 ownership ledger。
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(PipelineOwnershipState::default()),
            changed: Notify::new(),
        }
    }

    pub(crate) fn aggregator_snapshots(&self) -> usize {
        self.state.lock().aggregator_snapshots
    }

    pub(crate) fn set_aggregator_snapshots(&self, count: usize) {
        self.state.lock().aggregator_snapshots = count;
        self.changed.notify_waiters();
    }

    fn next_work_id(
        state: &mut PipelineOwnershipState,
    ) -> Result<BatchWorkId, PipelineOwnershipError> {
        let id = state.next_work_id;
        let next = id
            .checked_add(1)
            .ok_or(PipelineOwnershipError::InvalidTransition(
                "batch work ID space exhausted",
            ))?;
        state.next_work_id = next;
        Ok(BatchWorkId(id))
    }

    pub(crate) fn transfer_aggregator_to_exporter(
        &self,
        snapshots: usize,
    ) -> Result<BatchWorkId, PipelineOwnershipError> {
        let mut state = self.state.lock();
        let remaining = state.aggregator_snapshots.checked_sub(snapshots).ok_or(
            PipelineOwnershipError::InvalidTransition("aggregator ownership underflow"),
        )?;
        let id = Self::next_work_id(&mut state)?;
        state.aggregator_snapshots = remaining;
        state.exporter_batches.push_back(id);
        drop(state);
        self.changed.notify_waiters();
        Ok(id)
    }

    pub(crate) fn restore_exporter_to_aggregator(
        &self,
        id: BatchWorkId,
        snapshots: usize,
    ) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        if state.exporter_batches.back() != Some(&id) {
            return Err("exporter restore does not own the expected batch".into());
        }
        let restored = state.aggregator_snapshots.checked_add(snapshots).ok_or(
            PipelineOwnershipError::InvalidTransition("aggregator ownership overflow"),
        )?;
        state.exporter_batches.pop_back();
        state.aggregator_snapshots = restored;
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn exporter_batches(&self) -> usize {
        self.state.lock().exporter_batches.len()
    }

    pub(crate) fn exporter_front(&self) -> Option<BatchWorkId> {
        self.state.lock().exporter_batches.front().copied()
    }

    pub(crate) fn finish_exporter_batch(
        &self,
        id: BatchWorkId,
    ) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        if state.exporter_batches.front() != Some(&id) {
            return Err("exporter finish does not own the expected batch".into());
        }
        state.exporter_batches.pop_front();
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    #[cfg(any(feature = "client", test))]
    pub(crate) fn queue_client_batch(&self) -> Result<BatchWorkId, PipelineOwnershipError> {
        let mut state = self.state.lock();
        let id = Self::next_work_id(&mut state)?;
        state.client_queued_batches.push_back(id);
        drop(state);
        self.changed.notify_waiters();
        Ok(id)
    }

    pub(crate) fn queue_client_batches_front(
        &self,
        count: usize,
    ) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        let mut ids = Vec::with_capacity(count);
        for _ in 0..count {
            ids.push(Self::next_work_id(&mut state)?);
        }
        for id in ids.into_iter().rev() {
            state.client_queued_batches.push_front(id);
        }
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn client_front(&self) -> Option<BatchWorkId> {
        self.state.lock().client_queued_batches.front().copied()
    }

    #[cfg(any(feature = "client", test))]
    pub(crate) fn drop_client_batch(&self, id: BatchWorkId) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        let position = state
            .client_queued_batches
            .iter()
            .position(|candidate| *candidate == id)
            .ok_or(PipelineOwnershipError::InvalidTransition(
                "client drop does not own the expected batch",
            ))?;
        state.client_queued_batches.remove(position);
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    #[cfg(feature = "client")]
    pub(crate) fn transfer_exporter_to_client(
        &self,
        id: BatchWorkId,
    ) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        if state.exporter_batches.front() != Some(&id) {
            return Err("exporter handoff does not own the expected batch".into());
        }
        state.exporter_batches.pop_front();
        state.client_queued_batches.push_back(id);
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn client_queued_batches(&self) -> usize {
        self.state.lock().client_queued_batches.len()
    }

    pub(crate) fn transfer_client_to_pending_frames(
        &self,
        id: BatchWorkId,
    ) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        if state.client_queued_batches.front() != Some(&id) {
            return Err("client handoff does not own the expected batch".into());
        }
        state.client_queued_batches.pop_front();
        state.pending_frames.push_back(id);
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn pending_front(&self) -> Option<BatchWorkId> {
        self.state.lock().pending_frames.front().copied()
    }

    pub(crate) fn finish_pending_frame(
        &self,
        id: BatchWorkId,
    ) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        if state.pending_frames.front() != Some(&id) {
            return Err("pending-frame finish does not own the expected batch".into());
        }
        state.pending_frames.pop_front();
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn verify_pending_frames(
        &self,
        expected: usize,
    ) -> Result<(), PipelineOwnershipError> {
        if self.state.lock().pending_frames.len() != expected {
            return Err("pending-frame ownership does not match restored window".into());
        }
        Ok(())
    }

    #[cfg(feature = "client")]
    pub(crate) fn discard_pending_frames(
        &self,
        expected: usize,
    ) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        if state.pending_frames.len() != expected {
            return Err("pending-frame discard count does not match window".into());
        }
        state.pending_frames.clear();
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn pending_frames(&self) -> usize {
        self.state.lock().pending_frames.len()
    }

    #[cfg(any(feature = "client", test))]
    pub(crate) fn queue_command(&self, key: CommandWorkKey) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        let lifecycle = state.commands.entry(key).or_default();
        lifecycle.queued =
            lifecycle
                .queued
                .checked_add(1)
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "command queue ownership overflow",
                ))?;
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    #[cfg(any(feature = "client", test))]
    pub(crate) fn drop_command(&self, key: &CommandWorkKey) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        let lifecycle =
            state
                .commands
                .get_mut(key)
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "command ownership is missing",
                ))?;
        lifecycle.queued =
            lifecycle
                .queued
                .checked_sub(1)
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "command queue ownership is missing",
                ))?;
        if lifecycle.queued == 0 && !lifecycle.processing {
            state.commands.remove(key);
        }
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn start_command(
        &self,
        key: &CommandWorkKey,
    ) -> Result<CommandTransition, PipelineOwnershipError> {
        let mut state = self.state.lock();
        let lifecycle =
            state
                .commands
                .get_mut(key)
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "command ownership is missing",
                ))?;
        lifecycle.queued =
            lifecycle
                .queued
                .checked_sub(1)
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "command queue ownership is missing",
                ))?;
        let transition = if lifecycle.processing {
            CommandTransition::Coalesced
        } else {
            lifecycle.processing = true;
            CommandTransition::Started
        };
        drop(state);
        self.changed.notify_waiters();
        Ok(transition)
    }

    pub(crate) fn finish_command(
        &self,
        key: &CommandWorkKey,
    ) -> Result<(), PipelineOwnershipError> {
        let mut state = self.state.lock();
        let lifecycle =
            state
                .commands
                .get_mut(key)
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "command ownership is missing",
                ))?;
        if !lifecycle.processing {
            return Err("command is not processing".into());
        }
        lifecycle.processing = false;
        if lifecycle.queued == 0 {
            state.commands.remove(key);
        }
        drop(state);
        self.changed.notify_waiters();
        Ok(())
    }

    pub(crate) fn processing_commands(&self) -> usize {
        self.state
            .lock()
            .commands
            .values()
            .filter(|value| value.processing)
            .count()
    }

    pub(crate) fn transport_commands(&self) -> usize {
        self.state
            .lock()
            .commands
            .values()
            .map(|value| value.queued)
            .sum()
    }

    pub(crate) fn client_unflushed(&self) -> usize {
        let state = self.state.lock();
        state.client_queued_batches.len()
            + state.pending_frames.len()
            + state
                .commands
                .values()
                .map(|value| value.queued + usize::from(value.processing))
                .sum::<usize>()
    }

    pub(crate) fn total_unflushed(&self) -> usize {
        let state = self.state.lock();
        state.aggregator_snapshots
            + state.exporter_batches.len()
            + state.client_queued_batches.len()
            + state.pending_frames.len()
            + state
                .commands
                .values()
                .map(|value| value.queued + usize::from(value.processing))
                .sum::<usize>()
    }

    pub(crate) async fn changed(&self) {
        self.changed.notified().await;
    }
}

impl Default for PipelineOwnership {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandTransition, CommandWorkKey, PipelineOwnership};

    #[test]
    fn batch_work_ids_reject_double_finish_and_cross_stage_or_cross_id_transitions() {
        let ownership = PipelineOwnership::new();
        ownership.set_aggregator_snapshots(2);
        let first = ownership.transfer_aggregator_to_exporter(1).unwrap();
        let second = ownership.transfer_aggregator_to_exporter(1).unwrap();

        assert!(ownership.finish_exporter_batch(second).is_err());
        ownership.finish_exporter_batch(first).unwrap();
        assert!(ownership.finish_exporter_batch(first).is_err());
        ownership.finish_exporter_batch(second).unwrap();

        let client = ownership.queue_client_batch().unwrap();
        let later = ownership.queue_client_batch().unwrap();
        ownership.drop_client_batch(later).unwrap();
        assert!(ownership.drop_client_batch(later).is_err());
        assert!(ownership.finish_pending_frame(client).is_err());
        ownership.transfer_client_to_pending_frames(client).unwrap();
        ownership.finish_pending_frame(client).unwrap();
        assert!(ownership.finish_pending_frame(client).is_err());
        assert_eq!(ownership.total_unflushed(), 0);
    }

    #[test]
    fn transport_queued_command_is_visible_before_processing_and_decrements_once() {
        let ownership = PipelineOwnership::new();
        let key = CommandWorkKey::for_test("queued", b"payload");
        ownership.queue_command(key.clone()).unwrap();
        assert_eq!(ownership.total_unflushed(), 1);
        ownership.drop_command(&key).unwrap();
        assert!(ownership.drop_command(&key).is_err());
        assert_eq!(ownership.total_unflushed(), 0);

        ownership.queue_command(key.clone()).unwrap();
        ownership.start_command(&key).unwrap();
        assert_eq!(ownership.total_unflushed(), 1);

        ownership.finish_command(&key).unwrap();
        assert!(ownership.finish_command(&key).is_err());
        assert_eq!(ownership.total_unflushed(), 0);
    }

    #[test]
    fn keyed_command_replay_send_failure_and_overflow_cleanup_preserve_other_work() {
        let ownership = PipelineOwnership::new();
        let full = CommandWorkKey::for_test("full", b"full");
        let ordinary = CommandWorkKey::for_test("ordinary", b"reset");
        let overflow = CommandWorkKey::for_test("overflow", b"reset-2");

        ownership.queue_command(full.clone()).unwrap();
        assert_eq!(
            ownership.start_command(&full).unwrap(),
            CommandTransition::Started
        );
        ownership.queue_command(ordinary.clone()).unwrap();
        assert_eq!(
            ownership.start_command(&ordinary).unwrap(),
            CommandTransition::Started
        );
        assert_eq!(ownership.total_unflushed(), 2);

        // reply send 失败后 ordinary processing token 保留；重连重投只合并 queued token。
        ownership.queue_command(ordinary.clone()).unwrap();
        assert_eq!(
            ownership.start_command(&ordinary).unwrap(),
            CommandTransition::Coalesced
        );
        assert_eq!(ownership.total_unflushed(), 2);

        // deferred/successor overflow 只释放它自己的 queued token。
        ownership.queue_command(overflow.clone()).unwrap();
        ownership.drop_command(&overflow).unwrap();
        assert!(ownership.drop_command(&full).is_err());
        assert_eq!(ownership.total_unflushed(), 2);

        ownership.finish_command(&ordinary).unwrap();
        assert_eq!(ownership.total_unflushed(), 1, "full command remains owned");
        ownership.finish_command(&full).unwrap();
        assert_eq!(ownership.total_unflushed(), 0);
    }
}
