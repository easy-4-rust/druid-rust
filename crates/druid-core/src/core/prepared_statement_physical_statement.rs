//! PreparedStatement 的 Statement 基类适配。

use super::{
    DruidError, PhysicalPreparedStatement, PhysicalStatement, PhysicalStatementOptions, SqlWarning,
};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 将真实 `PhysicalPreparedStatement` 暴露为其继承的 `PhysicalStatement` 能力。
///
/// 对应 Java：`java.sql.PreparedStatement extends java.sql.Statement`。逻辑
/// `DruidPooledStatement` 关闭本适配器时只关闭基类视图，不关闭可能要回到缓存的
/// 物理 PreparedStatement；物理句柄由 holder/pool 生命周期单独管理。
pub(crate) struct PreparedStatementPhysicalStatement {
    statement: Arc<dyn PhysicalPreparedStatement>,
    options: PhysicalStatementOptions,
    closed: AtomicBool,
}

impl PreparedStatementPhysicalStatement {
    /// 创建共享同一真实 PreparedStatement 的 Statement 基类视图。
    pub(crate) fn new(
        statement: Arc<dyn PhysicalPreparedStatement>,
        options: PhysicalStatementOptions,
    ) -> Self {
        Self {
            statement,
            options,
            closed: AtomicBool::new(false),
        }
    }

    fn ensure_open(&self) -> Result<(), DruidError> {
        if self.is_closed() {
            Err(DruidError::Other("statement is closed".to_string()))
        } else {
            Ok(())
        }
    }
}

impl PhysicalStatement for PreparedStatementPhysicalStatement {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || self.statement.is_closed()
    }

    fn options(&self) -> PhysicalStatementOptions {
        self.options
    }

    fn max_field_size(&self) -> Result<i32, DruidError> {
        self.ensure_open()?;
        self.statement.max_field_size()
    }

    fn set_max_field_size(&self, max: i32) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.set_max_field_size(max)
    }

    fn max_rows(&self) -> Result<i32, DruidError> {
        self.ensure_open()?;
        self.statement.max_rows()
    }

    fn set_max_rows(&self, max: i32) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.set_max_rows(max)
    }

    fn set_escape_processing(&self, enabled: bool) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.set_escape_processing(enabled)
    }

    fn query_timeout(&self) -> Result<i32, DruidError> {
        self.ensure_open()?;
        self.statement.query_timeout()
    }

    fn set_query_timeout(&self, seconds: i32) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.set_query_timeout(seconds)
    }

    fn cancel(&self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.cancel()
    }

    fn warnings(&self) -> Result<Option<SqlWarning>, DruidError> {
        self.ensure_open()?;
        self.statement.warnings()
    }

    fn clear_warnings(&self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.clear_warnings()
    }

    fn set_cursor_name(&self, name: &str) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.set_cursor_name(name)
    }

    fn set_fetch_direction(&self, direction: i32) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.set_fetch_direction(direction)
    }

    fn fetch_direction(&self) -> Result<i32, DruidError> {
        self.ensure_open()?;
        self.statement.fetch_direction()
    }

    fn set_fetch_size(&self, rows: i32) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.set_fetch_size(rows)
    }

    fn fetch_size(&self) -> Result<i32, DruidError> {
        self.ensure_open()?;
        self.statement.fetch_size()
    }

    fn add_batch(&self, _sql: &str) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_add_batch_sql",
        })
    }

    fn clear_batch(&self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.clear_batch()
    }

    fn batch(&self) -> Result<Vec<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_batch_sql_snapshot",
        })
    }

    fn get_result_set(&self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.get_result_set()
    }

    fn get_update_count(&self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.get_update_count()
    }

    fn get_generated_keys(&self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.get_generated_keys()
    }

    fn get_more_results(&self, current: Option<i32>) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.get_more_results(current)
    }

    fn set_poolable(&self, poolable: bool) -> Result<(), DruidError> {
        self.ensure_open()?;
        if poolable {
            Ok(())
        } else {
            Err(DruidError::UnsupportedOperation {
                operation: "statement_set_poolable_false",
            })
        }
    }

    fn is_poolable(&self) -> bool {
        false
    }

    fn close_on_completion(&self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.statement.close_on_completion()
    }

    fn is_close_on_completion(&self) -> Result<bool, DruidError> {
        self.ensure_open()?;
        self.statement.is_close_on_completion()
    }
}
