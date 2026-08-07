use druid::core::{
    DruidError, PhysicalPreparedStatement, PhysicalStatementOptions, PreparedStatementKey,
};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};

/// JDBC Agent 的延迟物理预编译语句描述。
///
/// SQL 和 JDBC `ResultSet` 创建参数由 Druid 缓存；每次执行仍由 Agent 在同一
/// 物理 JDBC Connection 上创建真实 `PreparedStatement` 并绑定参数。
#[derive(Debug)]
pub struct JdbcAgentPreparedStatement {
    sql: String,
    options: PhysicalStatementOptions,
    closed: AtomicBool,
}

impl JdbcAgentPreparedStatement {
    pub(crate) fn new(key: &PreparedStatementKey) -> Self {
        Self {
            sql: key.sql().to_owned(),
            options: PhysicalStatementOptions {
                result_set_type: key.result_set_type(),
                result_set_concurrency: key.result_set_concurrency(),
                result_set_holdability: key.result_set_holdability(),
            },
            closed: AtomicBool::new(false),
        }
    }
}

impl PhysicalPreparedStatement for JdbcAgentPreparedStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn statement_options(&self) -> PhysicalStatementOptions {
        self.options
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
