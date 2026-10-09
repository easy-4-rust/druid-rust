use std::sync::atomic::AtomicUsize;

/// 固定原租约合同的十个真实委托入口计数。
#[derive(Default)]
pub(super) struct DelegationCounts {
    pub(super) create_physical_statement: AtomicUsize,
    pub(super) prepare_physical_statement: AtomicUsize,
    pub(super) prepare_physical_call: AtomicUsize,
    pub(super) exec_batch: AtomicUsize,
    pub(super) exec_prepared: AtomicUsize,
    pub(super) fetch_prepared: AtomicUsize,
    pub(super) database_meta_data: AtomicUsize,
    pub(super) create_blob: AtomicUsize,
    pub(super) create_clob: AtomicUsize,
    pub(super) create_n_clob: AtomicUsize,
}
