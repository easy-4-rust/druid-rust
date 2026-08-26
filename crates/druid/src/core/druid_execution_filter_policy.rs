//! Physical 与 canonical standard 共享的 SQL Filter around policy。

use super::{
    BatchExecContext, DruidError, ExecContext, ExecResult, FilterChain, RdbcError, StatementEvent,
};
use crate::stats::StatsCollector;
use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

/// 执行一次 before → raw → after Filter 栈，不拥有任何 raw/proxy 层级。
pub(crate) struct DruidExecutionFilterPolicy;

impl DruidExecutionFilterPolicy {
    pub(crate) fn prepare_statement_sql(
        filter_chain: Option<&FilterChain>,
        sql: &str,
    ) -> Result<String, DruidError> {
        filter_chain.map_or_else(
            || Ok(sql.to_owned()),
            |filter_chain| filter_chain.prepare_statement_sql(sql),
        )
    }

    pub(crate) fn statement_add_batch_sql(
        filter_chain: Option<&FilterChain>,
        sql: &str,
    ) -> Result<String, DruidError> {
        filter_chain.map_or_else(
            || Ok(sql.to_owned()),
            |filter_chain| filter_chain.statement_add_batch_sql(sql),
        )
    }

    pub(crate) async fn complete_statement_creation(
        filter_chain: Option<&FilterChain>,
        stats_collector: Option<&Arc<StatsCollector>>,
        connection_id: u64,
        statement_id: u64,
        event: &StatementEvent,
    ) -> Result<(), DruidError> {
        if let Some(filter_chain) = filter_chain {
            filter_chain
                .after_statement_event_with_identity(connection_id, statement_id, event)
                .await?;
        }
        if let Some(stats) = stats_collector {
            match event {
                StatementEvent::CreateStatement => {
                    stats.statement_stat().increment_create_counter();
                }
                StatementEvent::PrepareStatement(_) => {
                    stats.statement_stat().increment_prepare_counter();
                }
                StatementEvent::PrepareCall(_) => {
                    stats.statement_stat().increment_prepare_call_count();
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) async fn execute<T, F, Fut, S>(
        filter_chain: Option<&FilterChain>,
        context: &mut ExecContext<'_>,
        raw: F,
        summarize: S,
    ) -> Result<(String, Result<T, RdbcError>, Result<(), DruidError>), DruidError>
    where
        F: FnOnce(String) -> Fut,
        Fut: Future<Output = Result<T, RdbcError>>,
        S: Fn(&T) -> ExecResult,
    {
        if let Some(filter_chain) = filter_chain {
            filter_chain.before_execute(context).await?;
        }
        let effective_sql = context.sql.clone();
        let started = Instant::now();
        let result = raw(effective_sql.clone()).await;
        let after = match filter_chain {
            Some(filter_chain) => {
                let filter_result = result
                    .as_ref()
                    .map(&summarize)
                    .map_err(|error| DruidError::from((*error).clone()));
                filter_chain
                    .after_execute(context, &filter_result, started.elapsed())
                    .await
            }
            None => Ok(()),
        };
        Ok((effective_sql, result, after))
    }

    pub(crate) async fn execute_batch<T, F, Fut, S>(
        filter_chain: Option<&FilterChain>,
        context: &mut BatchExecContext<'_>,
        raw: F,
        summarize: S,
    ) -> Result<(Result<T, RdbcError>, Result<(), DruidError>), DruidError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, RdbcError>>,
        S: Fn(&T) -> Vec<i32>,
    {
        if let Some(filter_chain) = filter_chain {
            filter_chain.before_batch(context).await?;
        }
        let started = Instant::now();
        let result = raw().await;
        let after = match filter_chain {
            Some(filter_chain) => {
                let filter_result = result
                    .as_ref()
                    .map(&summarize)
                    .map_err(|error| DruidError::from((*error).clone()));
                filter_chain
                    .after_batch(context, &filter_result, started.elapsed())
                    .await
            }
            None => Ok(()),
        };
        Ok((result, after))
    }
}
