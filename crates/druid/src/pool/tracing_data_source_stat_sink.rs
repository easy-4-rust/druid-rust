use super::DataSourceStatSink;
use crate::core::DruidError;
use crate::stats::DruidDataSourceStatValue;

/// 将数据源统计快照发布为结构化 `tracing` 事件。
///
/// 对应 Java: `DruidDataSourceStatLoggerImpl` 的默认输出职责，但不迁移
/// SLF4J/Log4j/Commons Logging 类型、logger name 或 JSON 日志门面。
#[derive(Debug, Default)]
pub struct TracingDataSourceStatSink;

impl TracingDataSourceStatSink {
    /// 创建默认 tracing 统计输出端。
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl DataSourceStatSink for TracingDataSourceStatSink {
    fn publish(&self, stat_value: &DruidDataSourceStatValue) -> Result<(), DruidError> {
        let snapshot = serde_json::to_string(stat_value)
            .map_err(|error| DruidError::Other(format!("serialize datasource stats: {error}")))?;
        tracing::info!(
            target: "druid::stats",
            data_source = %stat_value.name,
            db_type = stat_value.db_type.as_deref().unwrap_or(""),
            url = stat_value.url.as_deref().unwrap_or(""),
            active_count = stat_value.active_count,
            pooling_count = stat_value.pooling_count,
            connect_count = stat_value.connect_count,
            close_count = stat_value.close_count,
            snapshot = %snapshot,
            "druid datasource statistics"
        );
        Ok(())
    }
}
