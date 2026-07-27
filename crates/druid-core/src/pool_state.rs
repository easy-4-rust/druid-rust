//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSourceStatValue

use std::time::Duration;

/// 连接池状态快照。
#[derive(Debug, Clone, Default)]
pub struct PoolState {
    pub name: String,
    pub driver_name: String,
    pub url: String,
    pub max_open: usize,
    pub active_count: usize,
    pub idle_count: usize,
    pub wait_count: usize,
    pub create_count: u64,
    pub close_count: u64,
    pub connect_count: u64,
    pub connect_error_count: u64,
    pub recycle_count: u64,
    pub discard_count: u64,
    pub leak_detection_count: u64,
    pub closed: bool,
    pub last_acquire_time: Option<Duration>,
}
