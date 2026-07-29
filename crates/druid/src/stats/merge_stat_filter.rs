//! 对应 Java：`com.alibaba.druid.filter.stat.MergeStatFilter`。

use super::{StatFilter, StatsCollector};
use crate::core::{
    AfterFilter, BatchExecContext, BeforeFilter, ConnectionEvent, DruidError, ExecContext,
    ExecResult, ResultSetFilter, ResultSetFilterChain, ResultSetFilterContext,
};
use std::sync::Arc;
use std::time::Duration;

/// 构造时强制开启 SQL 参数化合并的统计 Filter。
///
/// Java `MergeStatFilter` 继承 `StatFilter` 并在构造器中调用
/// `setMergeSql(true)`。Rust 使用组合保留同一业务语义，同时维持独立对象名称。
pub struct MergeStatFilter {
    stat_filter: StatFilter,
}

impl MergeStatFilter {
    /// 创建合并统计 Filter。
    #[must_use]
    pub fn new(collector: Arc<StatsCollector>) -> Self {
        let stat_filter = StatFilter::new(collector);
        stat_filter.set_merge_sql(true);
        Self { stat_filter }
    }

    /// 返回父级 `StatFilter` 视图，对应 Java `unwrap(StatFilter.class)`。
    #[must_use]
    pub fn as_stat_filter(&self) -> &StatFilter {
        &self.stat_filter
    }

    /// 返回是否启用 SQL 合并；构造后始终为 `true`。
    #[must_use]
    pub fn is_merge_sql(&self) -> bool {
        self.stat_filter.is_merge_sql()
    }
}

#[async_trait::async_trait]
impl BeforeFilter for MergeStatFilter {
    fn name(&self) -> &str {
        "mergeStat"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        self.stat_filter.before(context).await
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        self.stat_filter.before_batch(context).await
    }
}

#[async_trait::async_trait]
impl AfterFilter for MergeStatFilter {
    fn name(&self) -> &str {
        "mergeStat"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.stat_filter.after(context, result, elapsed).await
    }

    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.stat_filter.after_batch(context, result, elapsed).await
    }

    async fn after_connection_event(
        &self,
        event: &ConnectionEvent,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.stat_filter
            .after_connection_event(event, elapsed)
            .await
    }
}

impl ResultSetFilter for MergeStatFilter {
    fn result_set_open_after(&self, context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.stat_filter.result_set_open_after(context)
    }

    fn result_set_close(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<(), DruidError> {
        self.stat_filter.result_set_close(chain)
    }
}
