use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::pipeline_ownership::PipelineOwnership;

/// Internal runtime self-metrics (counters for the metrics pipeline itself).
#[derive(Debug)]
pub struct RuntimeSelfMetrics {
    /// Number of times `try_snapshot` returned Busy.
    snapshot_busy_total: AtomicU64,
    /// Number of snapshots dropped due to queue saturation (coalesced away).
    coalesced_total: AtomicU64,
    /// Whether the owned unregister listener task is still running.
    unregister_listener_active: AtomicBool,
    /// Drop 通知因有界队列已满而合并的次数。
    unregister_coalesced_total: AtomicU64,
    /// 当前仍 active 的 datasource registrations。
    registered_datasources: AtomicUsize,
    /// aggregator/exporter/client/command 的互斥 ownership ledger。
    ownership: Arc<PipelineOwnership>,
}

impl RuntimeSelfMetrics {
    pub fn new() -> Self {
        Self::with_ownership(Arc::new(PipelineOwnership::new()))
    }

    pub(crate) fn with_ownership(ownership: Arc<PipelineOwnership>) -> Self {
        Self {
            snapshot_busy_total: AtomicU64::new(0),
            coalesced_total: AtomicU64::new(0),
            unregister_listener_active: AtomicBool::new(false),
            unregister_coalesced_total: AtomicU64::new(0),
            registered_datasources: AtomicUsize::new(0),
            ownership,
        }
    }

    pub fn increment_snapshot_busy_total(&self) {
        self.snapshot_busy_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_coalesced_total(&self) {
        self.coalesced_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot_busy_total(&self) -> u64 {
        self.snapshot_busy_total.load(Ordering::Relaxed)
    }

    pub fn coalesced_total(&self) -> u64 {
        self.coalesced_total.load(Ordering::Relaxed)
    }

    pub fn pending_snapshots(&self) -> usize {
        self.ownership.aggregator_snapshots()
    }

    pub fn set_pending_snapshots(&self, n: usize) {
        self.ownership.set_aggregator_snapshots(n);
    }

    /// 返回注销监听任务是否仍在运行，用于关闭期可观测性。
    pub fn unregister_listener_active(&self) -> bool {
        self.unregister_listener_active.load(Ordering::Acquire)
    }

    pub(crate) fn set_unregister_listener_active(&self, active: bool) {
        self.unregister_listener_active
            .store(active, Ordering::Release);
    }

    /// 返回有界 unregister queue 的 coalesced 次数。
    pub fn unregister_coalesced_total(&self) -> u64 {
        self.unregister_coalesced_total.load(Ordering::Acquire)
    }

    pub(crate) fn increment_unregister_coalesced_total(&self) {
        self.unregister_coalesced_total
            .fetch_add(1, Ordering::AcqRel);
    }

    /// 返回当前 active registration 数。
    pub fn registered_datasources(&self) -> usize {
        self.registered_datasources.load(Ordering::Acquire)
    }

    pub(crate) fn set_registered_datasources(&self, count: usize) {
        self.registered_datasources.store(count, Ordering::Release);
    }

    /// 返回 exporter channel/task 当前持有的批次数。
    pub fn pending_export_batches(&self) -> usize {
        self.ownership.exporter_batches()
    }

    /// 在一个 batch 成功进入 exporter channel 后增加 backlog。
    pub fn increment_pending_export_batches(&self) {
        self.ownership.transfer_aggregator_to_exporter(0);
    }

    pub(crate) fn decrement_pending_export_batches(&self) {
        self.ownership.finish_exporter_batch();
    }

    /// 原子把指定 aggregator snapshot 数转成一个 exporter batch owner。
    pub fn transfer_aggregator_to_exporter(&self, snapshots: usize) {
        self.ownership.transfer_aggregator_to_exporter(snapshots);
    }

    /// exporter channel 关闭时原子把 batch ownership 还原为 snapshot owner。
    pub fn restore_exporter_to_aggregator(&self, snapshots: usize) {
        self.ownership.restore_exporter_to_aggregator(snapshots);
    }

    pub(crate) fn total_unflushed(&self) -> usize {
        self.ownership.total_unflushed()
    }

    /// 返回 aggregator、exporter、client 与 command 的精确互斥未刷新总数。
    #[must_use]
    pub fn unflushed_work(&self) -> usize {
        self.total_unflushed()
    }
}

impl Default for RuntimeSelfMetrics {
    fn default() -> Self {
        Self::new()
    }
}
