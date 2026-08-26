//! Physical 与 canonical standard 共享的 SQL Filter around policy。

use super::{DruidError, ExecContext, ExecResult, FilterChain, RdbcError};
use std::future::Future;
use std::time::Instant;

/// 执行一次 before → raw → after Filter 栈，不拥有任何 raw/proxy 层级。
pub(crate) struct DruidExecutionFilterPolicy;

impl DruidExecutionFilterPolicy {
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
}
