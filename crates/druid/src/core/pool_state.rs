//! 早期 Rust 兼容状态视图；Java 完整区间对象由
//! `stats::DruidDataSourceStatValue` 独立承载。

use std::time::Duration;

/// 连接池状态快照。
#[derive(Debug, Clone, Default)]
pub struct PoolState {
    pub name: String,
    pub driver_name: String,
    pub url: String,
    pub max_open: usize,
    pub active_count: usize,
    pub active_peak: usize,
    pub active_peak_time_millis: u64,
    pub idle_count: usize,
    pub pooling_peak: usize,
    pub pooling_peak_time_millis: u64,
    pub wait_count: usize,
    pub not_empty_wait_count: u64,
    pub not_empty_wait_nanos: u64,
    /// Java `maxWaitThreadCount`；`None` 对应 `-1`。
    pub max_wait_thread_count: Option<usize>,
    pub create_count: u64,
    /// 逻辑池化连接关闭次数，对应 Java `closeCount`。
    pub close_count: u64,
    /// 物理连接销毁次数，对应 Java `destroyCount`。
    pub destroy_count: u64,
    pub connect_count: u64,
    pub connect_error_count: u64,
    /// 物理连接创建、初始化或校验失败次数。
    pub physical_connect_error_count: u64,
    /// Java `isFailContinuous()`。
    pub fail_continuous: bool,
    /// 最近一次进入连续创建失败状态的 epoch millis；未处于失败时为 0。
    pub fail_continuous_time_millis: u64,
    /// Java `getLastCreateError()` 的稳定字符串视图。
    pub last_create_error: Option<String>,
    /// Java `getLastCreateErrorTimeMillis()`。
    pub last_create_error_time_millis: u64,
    pub recycle_count: u64,
    /// 回收过程发生异常的次数，对应 Java `recycleErrorCount`。
    pub recycle_error_count: u64,
    pub discard_count: u64,
    /// 空闲连接保活检查次数，对应 Java `keepAliveCheckCount`。
    pub keep_alive_check_count: u64,
    /// 空闲连接保活检查失败次数，对应 Java `keepAliveCheckErrorCount`。
    pub keep_alive_check_error_count: u64,
    /// 创建物理 PreparedStatement 总数。
    pub prepared_statement_count: u64,
    /// 关闭物理 PreparedStatement 总数。
    pub closed_prepared_statement_count: u64,
    /// 当前缓存 PreparedStatement 数。
    pub cached_prepared_statement_count: i64,
    /// 缓存删除次数。
    pub cached_prepared_statement_delete_count: u64,
    /// 缓存命中次数。
    pub cached_prepared_statement_hit_count: u64,
    /// 缓存未命中次数。
    pub cached_prepared_statement_miss_count: u64,
    /// 缓存访问次数。
    pub cached_prepared_statement_access_count: u64,
    pub leak_detection_count: u64,
    /// Java `resetStatEnable`。
    pub reset_stat_enable: bool,
    /// Java `resetCount`，仅实际执行 resetStat 时增加。
    pub reset_count: u64,
    pub closed: bool,
    pub last_acquire_time: Option<Duration>,
}
