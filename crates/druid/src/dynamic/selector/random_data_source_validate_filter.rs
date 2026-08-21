//! 对应 Java 类：`com.alibaba.druid.pool.ha.selector.RandomDataSourceValidateFilter`。

use super::RandomDataSourceValidateTask;
use crate::core::{
    AfterFilter, BeforeFilter, DruidError, ExecContext, ExecResult, ResultSetFilter,
};
use std::time::Duration;

/// 在 Statement 成功后记录数据源最近成功时间的 HA Filter。
#[derive(Debug, Default, Clone, Copy)]
pub struct RandomDataSourceValidateFilter;

#[async_trait::async_trait]
impl BeforeFilter for RandomDataSourceValidateFilter {
    fn name(&self) -> &str {
        "randomDataSourceValidate"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for RandomDataSourceValidateFilter {
    fn name(&self) -> &str {
        "randomDataSourceValidate"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        if result.is_ok() {
            RandomDataSourceValidateTask::log_success_time(context.data_source);
        }
        Ok(())
    }
}

impl ResultSetFilter for RandomDataSourceValidateFilter {}
