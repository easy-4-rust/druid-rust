//! 对应 Java 类：com.alibaba.druid.filter.stat.StatFilter
//!
//! 统计 `Filter`，实现 `AfterFilter` 接口。

use super::{collector::StatsCollector, StatFilterContext};
use crate::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, ConnectionEvent, DruidError,
    ExecContext, ExecOperation, ExecResult, ResultSetFilter, ResultSetFilterChain,
    ResultSetFilterContext,
};
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

    /// 返回本 `Filter` 所属数据源的 `ResultSet` 统计对象。
    pub fn result_set_stat(&self) -> &super::JdbcResultSetStat {
        self.collector.result_set_stat()
    }
}

#[async_trait::async_trait]
impl BeforeFilter for StatFilter {
    fn name(&self) -> &str {
        "stat"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        StatFilterContext::global().execute_before(context.sql, context.in_transaction)
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        self.collector
            // Java PreparedStatementProxyImpl 不填充继承的 batchSqlList，故为 0。
            .record_execute_batch(context.statements.len());
        StatFilterContext::global().execute_before(context.sql, context.in_transaction)
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
    ) -> Result<(), DruidError> {
        self.collector.record_sql(ctx.sql, elapsed, result.is_ok());
        let context = StatFilterContext::global();
        if ctx.operation == ExecOperation::Update {
            if let Ok(execution) = result {
                let update_count = i32::try_from(execution.rows_affected).unwrap_or(i32::MAX);
                context.add_update_count(update_count)?;
            }
        }
        let elapsed_nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        context.execute_after(Some(ctx.sql), elapsed_nanos, result.as_ref().err())
    }

    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.collector
            .record_sql(context.sql, elapsed, result.is_ok());
        let global = StatFilterContext::global();
        if let Ok(update_counts) = result {
            for update_count in update_counts {
                global.add_update_count(*update_count)?;
            }
        }
        let elapsed_nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        let sql = match (context.kind, result.is_ok()) {
            // Java PreparedStatementProxyImpl#getLastExecuteSql() 固定返回预编译 SQL。
            (BatchExecKind::PreparedStatement, _) => Some(context.sql),
            // 普通 Statement 成功 batch 不设置 lastExecuteSql；错误路径使用 batch SQL。
            (BatchExecKind::Statement, true) => None,
            (BatchExecKind::Statement, false) => Some(context.sql),
        };
        global.execute_after(sql, elapsed_nanos, result.as_ref().err())
    }

    async fn after_connection_event(
        &self,
        event: &ConnectionEvent,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        match event {
            ConnectionEvent::Commit => StatFilterContext::global().commit(),
            ConnectionEvent::Rollback => StatFilterContext::global().rollback(),
            _ => Ok(()),
        }
    }
}

impl ResultSetFilter for StatFilter {
    fn result_set_open_after(&self, context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.collector.result_set_stat().before_open();
        context.set_construct_time();
        StatFilterContext::global().result_set_open()
    }

    fn result_set_close(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<(), DruidError> {
        let elapsed = chain.context().elapsed().unwrap_or_default();
        let fetch_row_count = chain.context().fetch_row_count();
        let stat_elapsed_nanos = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        let listener_elapsed_nanos = i64::try_from(elapsed.as_nanos()).unwrap_or(i64::MAX);
        let stat = self.collector.result_set_stat();
        stat.after_close(stat_elapsed_nanos);
        stat.add_fetch_row_count(u64::try_from(fetch_row_count).unwrap_or_default());
        stat.increment_close_counter();
        StatFilterContext::global().add_fetch_row_count(fetch_row_count)?;
        chain.result_set_close()?;
        StatFilterContext::global().result_set_close(listener_elapsed_nanos)
    }
}
