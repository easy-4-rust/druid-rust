//! 对应 Java 类：com.alibaba.druid.filter.stat.StatFilter
//!
//! 统计 Filter，实现 AfterFilter 接口。

use crate::collector::StatsCollector;
use druid_core::{AfterFilter, DruidError, ExecContext, ExecResult};
use std::sync::Arc;
use std::time::Duration;

/// 统计 Filter。
///
/// 对应 Druid Java 的 `StatFilter`，在 SQL 执行后记录统计。
pub struct StatFilter {
    collector: Arc<StatsCollector>,
}

impl StatFilter {
    pub fn new(collector: Arc<StatsCollector>) -> Self {
        Self { collector }
    }
}

#[async_trait::async_trait]
impl AfterFilter for StatFilter {
    fn name(&self) -> &str {
        "stat"
    }

    async fn after(
        &self,
        ctx: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) {
        self.collector.record_sql(ctx.sql, elapsed, result.is_ok());
    }
}
