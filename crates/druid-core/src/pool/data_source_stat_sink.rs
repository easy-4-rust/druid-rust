use crate::core::DruidError;
use crate::stats::DruidDataSourceStatValue;

/// 数据源区间统计快照的 Rust 原生发布协议。
///
/// 对应 Java: `com.alibaba.druid.pool.DruidDataSourceStatLogger` 的产品语义。
/// Java `Log`、logger name 和 logger class 属于 JVM 日志实现边界，不进入本
/// trait；调用方可以把快照发送到 tracing、metrics、OpenTelemetry 或自定义
/// 管理系统。
pub trait DataSourceStatSink: Send + Sync {
    /// 发布一份已经执行 reset 的区间统计快照。
    ///
    /// # 参数
    ///
    /// - `stat_value`：本周期的不可变数据源统计值。
    ///
    /// # 返回
    ///
    /// 成功返回 `Ok(())`；失败返回结构化错误。周期任务会记录错误并继续下一轮，
    /// 对应 Java `LogStatsThread` 对单轮异常的隔离。
    fn publish(&self, stat_value: &DruidDataSourceStatValue) -> Result<(), DruidError>;
}
