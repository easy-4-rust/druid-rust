use druid_core::core::PoolState;
use serde::Serialize;

/// Wrapper 数据源的管理快照。
///
/// 迁移 Java DBCP/c3p0/Proxool `MBean` 的共同可观察字段。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapperPoolState {
    pub provider: String,
    pub name: String,
    pub driver_name: String,
    pub url: String,
    pub max_open: usize,
    pub active_count: usize,
    pub idle_count: usize,
    pub wait_count: usize,
    pub connect_count: u64,
    pub connect_error_count: u64,
    pub close_count: u64,
    pub recycle_count: u64,
    pub discard_count: u64,
    pub closed: bool,
}

impl WrapperPoolState {
    /// 从统一 `PoolState` 创建 wrapper 快照。
    #[must_use]
    pub fn from_pool_state(provider: impl Into<String>, state: PoolState) -> Self {
        Self {
            provider: provider.into(),
            name: state.name,
            driver_name: state.driver_name,
            url: state.url,
            max_open: state.max_open,
            active_count: state.active_count,
            idle_count: state.idle_count,
            wait_count: state.wait_count,
            connect_count: state.connect_count,
            connect_error_count: state.connect_error_count,
            close_count: state.close_count,
            recycle_count: state.recycle_count,
            discard_count: state.discard_count,
            closed: state.closed,
        }
    }
}
