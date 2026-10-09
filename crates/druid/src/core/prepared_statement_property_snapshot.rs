use super::druid_pooled_statement::DruidPooledStatement;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{FetchDirection, PreparedStatement, Statement};

/// 保留预编译语句的底层初始属性，关闭时按 Java 顺序恢复。
/// 对应 Java: com.alibaba.druid.pool.DruidPooledPreparedStatement#close
pub(crate) struct PreparedStatementPropertySnapshot {
    pub(crate) max_field_size: Option<i32>,
    pub(crate) max_rows: Option<i64>,
    pub(crate) query_timeout: Option<i32>,
    pub(crate) fetch_direction: Option<FetchDirection>,
    pub(crate) fetch_size: Option<i32>,
}

impl PreparedStatementPropertySnapshot {
    /// 读取可用的初始属性；不支持的属性不参与恢复，实际驱动错误保留。
    pub(crate) fn capture(statement: &dyn PreparedStatement) -> Result<Self, RdbcError> {
        fn optional<T>(value: Result<T, RdbcError>) -> Result<Option<T>, RdbcError> {
            match value {
                Ok(value) => Ok(Some(value)),
                Err(RdbcError::UnsupportedOperation { .. }) => Ok(None),
                Err(error) => Err(error),
            }
        }
        Ok(Self {
            max_field_size: optional(statement.max_field_size())?,
            max_rows: optional(statement.max_rows())?,
            query_timeout: optional(statement.query_timeout())?,
            fetch_direction: optional(statement.fetch_direction())?,
            fetch_size: optional(statement.fetch_size())?,
        })
    }

    /// 恢复已变更属性；首错立即返回，成功部分由代理状态保留，重试不重复恢复。
    pub(crate) fn restore(&self, statement: &mut DruidPooledStatement) -> Result<(), RdbcError> {
        if let Some(value) = self.max_field_size {
            if Statement::max_field_size(statement)? != value {
                Statement::set_max_field_size(statement, value)?;
            }
        }
        if let Some(value) = self.max_rows {
            if Statement::max_rows(statement)? != value {
                Statement::set_max_rows(statement, value)?;
            }
        }
        if let Some(value) = self.query_timeout {
            if statement.prepared_query_timeout() != value {
                Statement::set_query_timeout(statement, value)?;
            }
        }
        if let Some(value) = self.fetch_direction {
            if Statement::fetch_direction(statement)? != value {
                Statement::set_fetch_direction(statement, value)?;
            }
        }
        if let Some(value) = self.fetch_size {
            if Statement::fetch_size(statement)? != value {
                Statement::set_fetch_size(statement, value)?;
            }
        }
        Ok(())
    }
}
