use serde::Serialize;

/// JDBC Agent 运行时聚合指标快照；独立于 Druid `PoolState`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JdbcAgentRuntimeMetricsSnapshot {
    pub(crate) process_count: u64,
    pub(crate) active_sessions: u64,
    pub(crate) start_count: u64,
    pub(crate) crash_count: u64,
    pub(crate) rpc_count: u64,
    pub(crate) rpc_error_count: u64,
    pub(crate) rpc_latency_micros_total: u64,
    pub(crate) rpc_latency_micros_max: u64,
    pub(crate) timeout_count: u64,
    pub(crate) cancellation_count: u64,
    pub(crate) protocol_error_count: u64,
}

impl JdbcAgentRuntimeMetricsSnapshot {
    /// 返回当前 Agent 子进程数。
    #[must_use]
    pub const fn process_count(&self) -> u64 {
        self.process_count
    }

    /// 返回当前活跃 session 数。
    #[must_use]
    pub const fn active_sessions(&self) -> u64 {
        self.active_sessions
    }

    /// 返回累计启动数。
    #[must_use]
    pub const fn start_count(&self) -> u64 {
        self.start_count
    }

    /// 返回累计崩溃数。
    #[must_use]
    pub const fn crash_count(&self) -> u64 {
        self.crash_count
    }

    /// 返回累计 RPC 数。
    #[must_use]
    pub const fn rpc_count(&self) -> u64 {
        self.rpc_count
    }

    /// 返回累计 RPC 错误数。
    #[must_use]
    pub const fn rpc_error_count(&self) -> u64 {
        self.rpc_error_count
    }

    /// 返回累计 RPC 延迟微秒。
    #[must_use]
    pub const fn rpc_latency_micros_total(&self) -> u64 {
        self.rpc_latency_micros_total
    }

    /// 返回最大 RPC 延迟微秒。
    #[must_use]
    pub const fn rpc_latency_micros_max(&self) -> u64 {
        self.rpc_latency_micros_max
    }

    /// 返回累计超时数。
    #[must_use]
    pub const fn timeout_count(&self) -> u64 {
        self.timeout_count
    }

    /// 返回累计取消请求数。
    #[must_use]
    pub const fn cancellation_count(&self) -> u64 {
        self.cancellation_count
    }

    /// 返回累计协议错误数。
    #[must_use]
    pub const fn protocol_error_count(&self) -> u64 {
        self.protocol_error_count
    }
}
