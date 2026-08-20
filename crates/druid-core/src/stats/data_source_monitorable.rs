use serde_json::Value;

use super::{DataSourceIdentity, DruidTelemetrySnapshot, SnapshotUnavailable};

/// 可注册到 Druid 管理面的数据源统计契约。
///
/// 对应 Java：`com.alibaba.druid.stat.DataSourceMonitorable`，并把 Java
/// 反射调用的数据源管理方法显式化为 Rust trait。
pub trait DataSourceMonitorable: Send + Sync {
    /// 返回数据源名称。
    fn name(&self) -> &str;

    /// 返回 Rust 物理驱动/Adapter 名称，供 basic 管理协议枚举驱动。
    fn driver_name(&self) -> Option<&str> {
        None
    }

    /// 返回 datasource 管理协议对象。
    fn data_source_stat_data(&self) -> Value;

    /// 返回 SQL 统计数组。
    fn sql_stat_data(&self) -> Vec<Value> {
        Vec::new()
    }

    /// 返回 Wall 统计对象。
    fn wall_stat_data(&self) -> Value {
        Value::Object(serde_json::Map::new())
    }

    /// 返回当前空闲池连接信息。
    fn pooling_connection_info(&self) -> Vec<Value> {
        Vec::new()
    }

    /// 返回借出但尚未归还连接的调用栈。
    fn active_connection_stack_trace(&self) -> Vec<String> {
        Vec::new()
    }

    /// 返回是否启用 abandoned connection 追踪。
    fn is_remove_abandoned(&self) -> bool {
        false
    }

    /// 重置可重置的累计统计。
    fn reset_stat(&self);

    /// 重置本数据源独立的 `RdbcDataSourceStat`。
    fn reset_rdbc_stat(&self) {}

    /// 发布并重置一份区间统计；单个 sink 错误不得中断其他数据源。
    fn log_stats(&self) -> Result<(), crate::core::DruidError> {
        Ok(())
    }

    /// 返回数据源的稳定身份标识。
    fn identity(&self) -> DataSourceIdentity;

    /// 尝试获取一个非阻塞的类型化遥测快照。
    ///
    /// 如果数据源正忙（例如持有锁），返回 `Err(SnapshotUnavailable::Busy)`；
    /// 如果数据源已关闭，返回 `Err(SnapshotUnavailable::Closed)`。
    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable>;
}
