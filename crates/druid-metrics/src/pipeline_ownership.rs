use parking_lot::Mutex;
use tokio::sync::Notify;

#[derive(Debug, Default)]
struct PipelineOwnershipState {
    aggregator_snapshots: usize,
    exporter_batches: usize,
    client_queued_batches: usize,
    pending_frames: usize,
    transport_commands: usize,
    processing_commands: usize,
}

/// Runtime pipeline 各阶段互斥所有权的单一计数事实源。
///
/// Rust-only 并发对象；所有 handoff 都在一个锁内同时减旧 owner、加新 owner，
/// shutdown 因而不会观察到重叠或空窗。
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

    pub(crate) fn transfer_aggregator_to_exporter(&self, snapshots: usize) {
        let mut state = self.state.lock();
        debug_assert!(state.aggregator_snapshots >= snapshots);
        state.aggregator_snapshots = state.aggregator_snapshots.saturating_sub(snapshots);
        state.exporter_batches = state.exporter_batches.saturating_add(1);
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn restore_exporter_to_aggregator(&self, snapshots: usize) {
        let mut state = self.state.lock();
        debug_assert!(state.exporter_batches > 0);
        state.exporter_batches = state.exporter_batches.saturating_sub(1);
        state.aggregator_snapshots = state.aggregator_snapshots.saturating_add(snapshots);
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn exporter_batches(&self) -> usize {
        self.state.lock().exporter_batches
    }

    pub(crate) fn finish_exporter_batch(&self) {
        let mut state = self.state.lock();
        debug_assert!(state.exporter_batches > 0);
        state.exporter_batches = state.exporter_batches.saturating_sub(1);
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn queue_client_batch(&self) {
        let mut state = self.state.lock();
        state.client_queued_batches = state.client_queued_batches.saturating_add(1);
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn undo_client_batch(&self) {
        let mut state = self.state.lock();
        debug_assert!(state.client_queued_batches > 0);
        state.client_queued_batches = state.client_queued_batches.saturating_sub(1);
        drop(state);
        self.changed.notify_waiters();
    }

    #[cfg(feature = "client")]
    pub(crate) fn transfer_exporter_to_client(&self) {
        let mut state = self.state.lock();
        debug_assert!(state.exporter_batches > 0);
        state.exporter_batches = state.exporter_batches.saturating_sub(1);
        state.client_queued_batches = state.client_queued_batches.saturating_add(1);
        drop(state);
        self.changed.notify_waiters();
    }

    #[cfg(feature = "client")]
    pub(crate) fn restore_client_to_exporter(&self) {
        let mut state = self.state.lock();
        debug_assert!(state.client_queued_batches > 0);
        state.client_queued_batches = state.client_queued_batches.saturating_sub(1);
        state.exporter_batches = state.exporter_batches.saturating_add(1);
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn client_queued_batches(&self) -> usize {
        self.state.lock().client_queued_batches
    }

    pub(crate) fn transfer_client_to_pending_frames(&self, pending_frames: usize) {
        let mut state = self.state.lock();
        debug_assert!(state.client_queued_batches > 0);
        state.client_queued_batches = state.client_queued_batches.saturating_sub(1);
        state.pending_frames = pending_frames;
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn set_pending_frames(&self, pending_frames: usize) {
        self.state.lock().pending_frames = pending_frames;
        self.changed.notify_waiters();
    }

    pub(crate) fn pending_frames(&self) -> usize {
        self.state.lock().pending_frames
    }

    #[cfg(any(feature = "client", test))]
    pub(crate) fn queue_transport_command(&self) {
        let mut state = self.state.lock();
        state.transport_commands = state.transport_commands.saturating_add(1);
        drop(state);
        self.changed.notify_waiters();
    }

    #[cfg(any(feature = "client", test))]
    pub(crate) fn drop_transport_command(&self) {
        let mut state = self.state.lock();
        state.transport_commands = state.transport_commands.saturating_sub(1);
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn transfer_transport_command_to_processing(&self) {
        let mut state = self.state.lock();
        state.transport_commands = state.transport_commands.saturating_sub(1);
        state.processing_commands = state.processing_commands.saturating_add(1);
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn finish_processing_command(&self) {
        let mut state = self.state.lock();
        state.processing_commands = state.processing_commands.saturating_sub(1);
        drop(state);
        self.changed.notify_waiters();
    }

    pub(crate) fn processing_commands(&self) -> usize {
        self.state.lock().processing_commands
    }

    pub(crate) fn transport_commands(&self) -> usize {
        self.state.lock().transport_commands
    }

    pub(crate) fn client_unflushed(&self) -> usize {
        let state = self.state.lock();
        state
            .client_queued_batches
            .saturating_add(state.pending_frames)
            .saturating_add(state.transport_commands)
            .saturating_add(state.processing_commands)
    }

    pub(crate) fn total_unflushed(&self) -> usize {
        let state = self.state.lock();
        state
            .aggregator_snapshots
            .saturating_add(state.exporter_batches)
            .saturating_add(state.client_queued_batches)
            .saturating_add(state.pending_frames)
            .saturating_add(state.transport_commands)
            .saturating_add(state.processing_commands)
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
    use super::PipelineOwnership;

    #[test]
    fn transport_queued_command_is_visible_before_processing_and_decrements_once() {
        let ownership = PipelineOwnership::new();
        ownership.queue_transport_command();
        assert_eq!(ownership.total_unflushed(), 1);
        ownership.drop_transport_command();
        assert_eq!(ownership.total_unflushed(), 0);

        ownership.queue_transport_command();
        ownership.transfer_transport_command_to_processing();
        assert_eq!(ownership.total_unflushed(), 1);

        ownership.finish_processing_command();
        ownership.finish_processing_command();
        assert_eq!(
            ownership.total_unflushed(),
            0,
            "reply/error/cancellation cleanup must not decrement a command twice"
        );
    }
}
