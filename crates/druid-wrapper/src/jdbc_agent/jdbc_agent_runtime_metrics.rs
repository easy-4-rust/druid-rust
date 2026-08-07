use super::JdbcAgentRuntimeMetricsSnapshot;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

static PROCESS_COUNT: AtomicU64 = AtomicU64::new(0);
static ACTIVE_SESSIONS: AtomicU64 = AtomicU64::new(0);
static START_COUNT: AtomicU64 = AtomicU64::new(0);
static CRASH_COUNT: AtomicU64 = AtomicU64::new(0);
static RPC_COUNT: AtomicU64 = AtomicU64::new(0);
static RPC_ERROR_COUNT: AtomicU64 = AtomicU64::new(0);
static RPC_LATENCY_MICROS_TOTAL: AtomicU64 = AtomicU64::new(0);
static RPC_LATENCY_MICROS_MAX: AtomicU64 = AtomicU64::new(0);
static TIMEOUT_COUNT: AtomicU64 = AtomicU64::new(0);
static CANCELLATION_COUNT: AtomicU64 = AtomicU64::new(0);
static PROTOCOL_ERROR_COUNT: AtomicU64 = AtomicU64::new(0);

/// JDBC Agent 进程、session 与 RPC 指标注册表。
pub struct JdbcAgentRuntimeMetrics;

impl JdbcAgentRuntimeMetrics {
    /// 返回当前进程内的无副作用聚合快照。
    #[must_use]
    pub fn snapshot() -> JdbcAgentRuntimeMetricsSnapshot {
        JdbcAgentRuntimeMetricsSnapshot {
            process_count: PROCESS_COUNT.load(Ordering::Acquire),
            active_sessions: ACTIVE_SESSIONS.load(Ordering::Acquire),
            start_count: START_COUNT.load(Ordering::Acquire),
            crash_count: CRASH_COUNT.load(Ordering::Acquire),
            rpc_count: RPC_COUNT.load(Ordering::Acquire),
            rpc_error_count: RPC_ERROR_COUNT.load(Ordering::Acquire),
            rpc_latency_micros_total: RPC_LATENCY_MICROS_TOTAL.load(Ordering::Acquire),
            rpc_latency_micros_max: RPC_LATENCY_MICROS_MAX.load(Ordering::Acquire),
            timeout_count: TIMEOUT_COUNT.load(Ordering::Acquire),
            cancellation_count: CANCELLATION_COUNT.load(Ordering::Acquire),
            protocol_error_count: PROTOCOL_ERROR_COUNT.load(Ordering::Acquire),
        }
    }

    pub(crate) fn process_started() {
        PROCESS_COUNT.fetch_add(1, Ordering::AcqRel);
        START_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn process_stopped() {
        Self::saturating_decrement(&PROCESS_COUNT);
    }

    pub(crate) fn session_opened() {
        ACTIVE_SESSIONS.fetch_add(1, Ordering::AcqRel);
    }

    pub(crate) fn session_closed() {
        Self::saturating_decrement(&ACTIVE_SESSIONS);
    }

    pub(crate) fn process_crashed() {
        CRASH_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn rpc_completed(elapsed: Duration, failed: bool) {
        RPC_COUNT.fetch_add(1, Ordering::Relaxed);
        if failed {
            RPC_ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        let micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
        RPC_LATENCY_MICROS_TOTAL.fetch_add(micros, Ordering::Relaxed);
        RPC_LATENCY_MICROS_MAX.fetch_max(micros, Ordering::Relaxed);
    }

    pub(crate) fn request_timed_out() {
        TIMEOUT_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn cancellation_requested() {
        CANCELLATION_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn protocol_error() {
        PROTOCOL_ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    fn saturating_decrement(counter: &AtomicU64) {
        let _ = counter.fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
            value.checked_sub(1)
        });
    }
}
