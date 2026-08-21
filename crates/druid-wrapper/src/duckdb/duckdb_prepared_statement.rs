//! `DuckDB` 原生预编译语句句柄。

use druid::core::{
    DruidError, PhysicalPreparedStatement, PhysicalStatementOptions, SqlTextPreparedStatement,
};
use duckdb::InterruptHandle;
use std::any::Any;
use std::sync::Arc;

/// `DuckDB` 连接级预编译语句描述。
///
/// duckdb-rs 的 `Statement` 借用 `Connection` 且既非 `Send` 也非 `Sync`，不能
/// 跨 Druid 异步资源边界保存。本对象记录经过原生 `prepare_cached` 校验的 SQL
/// 与连接身份；每次执行在同一物理连接的 statement cache 中重新取得句柄。
pub struct DuckDbPreparedStatement {
    connection_id: u64,
    statement: SqlTextPreparedStatement,
    interrupt_handle: Arc<InterruptHandle>,
}

impl DuckDbPreparedStatement {
    /// 创建已经由 `DuckDB` 原生 prepare 校验的语句描述。
    pub(crate) fn new(
        connection_id: u64,
        sql: impl Into<String>,
        interrupt_handle: Arc<InterruptHandle>,
    ) -> Self {
        Self {
            connection_id,
            statement: SqlTextPreparedStatement::new(sql),
            interrupt_handle,
        }
    }

    /// 返回创建该语句的物理连接身份。
    #[must_use]
    pub(crate) const fn connection_id(&self) -> u64 {
        self.connection_id
    }
}

impl PhysicalPreparedStatement for DuckDbPreparedStatement {
    fn sql(&self) -> &str {
        self.statement.sql()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn statement_options(&self) -> PhysicalStatementOptions {
        self.statement.statement_options()
    }

    fn max_field_size(&self) -> Result<i32, DruidError> {
        self.statement.max_field_size()
    }

    fn set_max_field_size(&self, max: i32) -> Result<(), DruidError> {
        self.statement.set_max_field_size(max)
    }

    fn max_rows(&self) -> Result<i32, DruidError> {
        self.statement.max_rows()
    }

    fn set_max_rows(&self, max: i32) -> Result<(), DruidError> {
        self.statement.set_max_rows(max)
    }

    fn set_escape_processing(&self, enabled: bool) -> Result<(), DruidError> {
        self.statement.set_escape_processing(enabled)
    }

    fn query_timeout(&self) -> Result<i32, DruidError> {
        self.statement.query_timeout()
    }

    fn set_query_timeout(&self, seconds: i32) -> Result<(), DruidError> {
        self.statement.set_query_timeout(seconds)
    }

    fn cancel(&self) -> Result<(), DruidError> {
        self.interrupt_handle.interrupt();
        Ok(())
    }

    fn set_cursor_name(&self, name: &str) -> Result<(), DruidError> {
        self.statement.set_cursor_name(name)
    }

    fn set_fetch_direction(&self, direction: i32) -> Result<(), DruidError> {
        self.statement.set_fetch_direction(direction)
    }

    fn fetch_direction(&self) -> Result<i32, DruidError> {
        self.statement.fetch_direction()
    }

    fn set_fetch_size(&self, rows: i32) -> Result<(), DruidError> {
        self.statement.set_fetch_size(rows)
    }

    fn fetch_size(&self) -> Result<i32, DruidError> {
        self.statement.fetch_size()
    }

    fn clear_warnings(&self) -> Result<(), DruidError> {
        self.statement.clear_warnings()
    }

    fn close_on_completion(&self) -> Result<(), DruidError> {
        self.statement.close_on_completion()
    }

    fn is_close_on_completion(&self) -> Result<bool, DruidError> {
        self.statement.is_close_on_completion()
    }

    fn close(&self) -> Result<(), DruidError> {
        self.statement.close()
    }

    fn is_closed(&self) -> bool {
        self.statement.is_closed()
    }
}
