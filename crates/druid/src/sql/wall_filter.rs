use super::{WallConfig, WallProvider};
use crate::core::{
    AfterFilter, BatchExecContext, BeforeFilter, DruidError, ExecContext, ExecResult,
    ResultSetFilter,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 把 WallProvider 接入 Druid FilterChain 的 canonical Filter。
///
/// 对应 Java：`com.alibaba.druid.wall.WallFilter`。违规可按配置记录日志，并可
/// 选择抛出 `WallViolation` 阻断执行；成功/失败及影响行数回写同一 SQL 统计。
pub struct WallFilter {
    provider: Arc<WallProvider>,
    log_violation: AtomicBool,
    throw_exception: AtomicBool,
}

impl WallFilter {
    /// 使用指定 provider 创建 Filter。
    #[must_use]
    pub fn new(provider: Arc<WallProvider>) -> Self {
        Self {
            provider,
            log_violation: AtomicBool::new(false),
            throw_exception: AtomicBool::new(true),
        }
    }

    /// 使用默认规则创建 Filter。
    #[must_use]
    pub fn with_config(config: WallConfig) -> Self {
        Self::new(Arc::new(WallProvider::new(config)))
    }

    /// 返回 provider。
    #[must_use]
    pub fn provider(&self) -> &Arc<WallProvider> {
        &self.provider
    }

    /// 设置是否记录违规。
    pub fn set_log_violation(&self, log_violation: bool) {
        self.log_violation.store(log_violation, Ordering::Release);
    }

    /// 设置是否以错误阻断违规 SQL。
    pub fn set_throw_exception(&self, throw_exception: bool) {
        self.throw_exception
            .store(throw_exception, Ordering::Release);
    }

    fn before_sql(&self, sql: &str) -> Result<(), DruidError> {
        let result = self.provider.check(sql);
        if let Some(violation) = result.violations().first() {
            if self.log_violation.load(Ordering::Acquire) {
                tracing::error!(sql, violation = %violation, "wall violation");
            }
            if self.throw_exception.load(Ordering::Acquire) {
                return Err(DruidError::WallViolation(violation.to_string()));
            }
        }
        result.sql_stat().increment_execute_count();
        Ok(())
    }

    fn after_sql(&self, sql: &str, result: &Result<ExecResult, DruidError>) {
        let Some(stat) = self.provider.sql_stat(sql) else {
            return;
        };
        match result {
            Ok(result) => {
                stat.add_update_count(result.rows_affected);
                if let Some(row_count) = result.row_count {
                    stat.add_fetch_row_count(row_count);
                }
                self.provider
                    .record_effect_rows(sql, result.rows_affected, result.row_count);
                if !stat.violations().is_empty() {
                    self.provider
                        .add_violation_effect_row_count(result.rows_affected);
                }
            }
            Err(_) => {
                stat.increment_execute_error_count();
            }
        }
    }
}

impl Default for WallFilter {
    fn default() -> Self {
        Self::with_config(WallConfig::default())
    }
}

#[async_trait::async_trait]
impl BeforeFilter for WallFilter {
    fn name(&self) -> &str {
        "wall"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        self.before_sql(context.sql)
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        if context.statements.is_empty() {
            return self.before_sql(context.sql);
        }
        for sql in context.statements {
            self.before_sql(sql)?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for WallFilter {
    fn name(&self) -> &str {
        "wall"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.after_sql(context.sql, result);
        Ok(())
    }

    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        let sqls: Vec<&str> = if context.statements.is_empty() {
            vec![context.sql]
        } else {
            context.statements.iter().map(String::as_str).collect()
        };
        match result {
            Ok(update_counts) => {
                for (index, sql) in sqls.iter().enumerate() {
                    let rows_affected = update_counts
                        .get(index)
                        .and_then(|count| u64::try_from(*count).ok())
                        .unwrap_or_default();
                    self.after_sql(
                        sql,
                        &Ok(ExecResult {
                            rows_affected,
                            last_insert_id: None,
                            row_count: None,
                        }),
                    );
                }
            }
            Err(error) => {
                for sql in sqls {
                    self.after_sql(sql, &Err(error.clone()));
                }
            }
        }
        Ok(())
    }
}

impl ResultSetFilter for WallFilter {}
