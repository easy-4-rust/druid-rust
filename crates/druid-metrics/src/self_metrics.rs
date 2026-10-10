use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::pipeline_ownership::{BatchWorkId, PipelineOwnership, PipelineOwnershipError};

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
    /// 当前由 supervisor 持有且尚未结束的后台任务数。
    active_tasks: AtomicUsize,
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
            active_tasks: AtomicUsize::new(0),
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

    /// 返回仍由 runtime 监督的后台任务数。
    pub fn active_tasks(&self) -> usize {
        self.active_tasks.load(Ordering::Acquire)
    }

    /// 登记一个新后台任务，包括尚未首次被调度的任务。
    pub(crate) fn task_started(&self) {
        self.active_tasks.fetch_add(1, Ordering::AcqRel);
    }

    /// 后台任务结束或被终止时撤销登记。
    pub(crate) fn task_stopped(&self) {
        self.active_tasks.fetch_sub(1, Ordering::AcqRel);
    }

    /// 返回 exporter channel/task 当前持有的批次数。
    pub fn pending_export_batches(&self) -> usize {
        self.ownership.exporter_batches()
    }

    pub(crate) fn decrement_pending_export_batches(&self) -> Result<(), PipelineOwnershipError> {
        let id =
            self.ownership
                .exporter_front()
                .ok_or(PipelineOwnershipError::InvalidTransition(
                    "exporter ownership is missing",
                ))?;
        self.ownership.finish_exporter_batch(id)
    }

    /// 原子把指定 aggregator snapshot 数转成一个 exporter batch owner。
    pub fn transfer_aggregator_to_exporter(&self, snapshots: usize) -> Result<u64, &'static str> {
        self.ownership
            .transfer_aggregator_to_exporter(snapshots)
            .map(BatchWorkId::into_raw)
            .map_err(PipelineOwnershipError::message)
    }

    /// exporter channel 关闭时原子把 batch ownership 还原为 snapshot owner。
    pub fn restore_exporter_to_aggregator(
        &self,
        id: u64,
        snapshots: usize,
    ) -> Result<(), &'static str> {
        self.ownership
            .restore_exporter_to_aggregator(BatchWorkId::from_raw(id), snapshots)
            .map_err(PipelineOwnershipError::message)
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
