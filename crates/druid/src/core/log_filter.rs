use super::{AfterFilter, BeforeFilter, DruidError, ExecContext, ExecResult, ResultSetFilter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Java 日志 Filter 家族的 tracing 实现。
///
/// 对应 Java：`com.alibaba.druid.filter.logging.LogFilter`。Log4j/Log4j2/
/// CommonsLogging/Slf4j 的后端差异在 Rust 中规划合并到 `tracing`，SQL、
/// 参数、执行结果和错误的可观察语义保持一致。
pub struct LogFilter {
    statement_log_enabled: AtomicBool,
    statement_parameter_log_enabled: AtomicBool,
    statement_executable_sql_log_enabled: AtomicBool,
    statement_log_error_enabled: AtomicBool,
}

impl LogFilter {
    /// 创建 Java 默认开关对应的日志 Filter。
    #[must_use]
    pub fn new() -> Self {
        Self {
            statement_log_enabled: AtomicBool::new(true),
            statement_parameter_log_enabled: AtomicBool::new(true),
            statement_executable_sql_log_enabled: AtomicBool::new(false),
            statement_log_error_enabled: AtomicBool::new(true),
        }
    }

    /// 设置是否记录 statement SQL。
    pub fn set_statement_log_enabled(&self, enabled: bool) {
        self.statement_log_enabled.store(enabled, Ordering::Release);
    }

    /// 设置是否记录参数。
    pub fn set_statement_parameter_log_enabled(&self, enabled: bool) {
        self.statement_parameter_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 设置是否记录可执行 SQL 标志。
    ///
    /// Rust 不拼接参数到 SQL，避免改变转义语义；开启后用独立结构化字段记录。
    pub fn set_statement_executable_sql_log_enabled(&self, enabled: bool) {
        self.statement_executable_sql_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 设置是否记录执行错误。
    pub fn set_statement_log_error_enabled(&self, enabled: bool) {
        self.statement_log_error_enabled
            .store(enabled, Ordering::Release);
    }
}

impl Default for LogFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl BeforeFilter for LogFilter {
    fn name(&self) -> &str {
        "log"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        if self.statement_log_enabled.load(Ordering::Acquire) {
            if self.statement_parameter_log_enabled.load(Ordering::Acquire) {
                tracing::debug!(
                    data_source = context.data_source,
                    sql = context.sql,
                    parameters = ?context.params,
                    executable_sql = self.statement_executable_sql_log_enabled.load(Ordering::Acquire),
                    "statement execute before"
                );
            } else {
                tracing::debug!(
                    data_source = context.data_source,
                    sql = context.sql,
                    "statement execute before"
                );
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for LogFilter {
    fn name(&self) -> &str {
        "log"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        match result {
            Ok(result) if self.statement_log_enabled.load(Ordering::Acquire) => {
                tracing::debug!(
                    data_source = context.data_source,
                    sql = context.sql,
                    elapsed_ms = elapsed.as_millis(),
                    rows_affected = result.rows_affected,
                    row_count = result.row_count,
                    "statement execute after"
                );
            }
            Err(error) if self.statement_log_error_enabled.load(Ordering::Acquire) => {
                tracing::error!(
                    data_source = context.data_source,
                    sql = context.sql,
                    elapsed_ms = elapsed.as_millis(),
                    %error,
                    "statement execute error"
                );
            }
            _ => {}
        }
        Ok(())
    }
}

impl ResultSetFilter for LogFilter {}
