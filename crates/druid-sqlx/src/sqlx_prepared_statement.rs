//! SQLx 预编译语句适配器。

use druid_core::{DruidError, PhysicalPreparedStatement};
use sqlx::any::AnyStatement;
use sqlx::sqlite::SqliteStatement;
use sqlx::Statement;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};

enum SqlxPreparedStatementBackend {
    Any(AnyStatement<'static>),
    Sqlite(SqliteStatement<'static>),
}

/// SQLx 驱动 prepare 结果。
///
/// 对应 Java 平台依赖：具体 JDBC driver 的 `PreparedStatement`。SQLx statement
/// 可跨同类连接复用，并由 SQLx connection 自身维护 server-side statement cache。
pub struct SqlxPreparedStatement {
    sql: String,
    backend: SqlxPreparedStatementBackend,
    closed: AtomicBool,
}

impl SqlxPreparedStatement {
    pub(crate) fn any(sql: impl Into<String>, statement: AnyStatement<'static>) -> Self {
        Self {
            sql: sql.into(),
            backend: SqlxPreparedStatementBackend::Any(statement),
            closed: AtomicBool::new(false),
        }
    }

    pub(crate) fn sqlite(sql: impl Into<String>, statement: SqliteStatement<'static>) -> Self {
        Self {
            sql: sql.into(),
            backend: SqlxPreparedStatementBackend::Sqlite(statement),
            closed: AtomicBool::new(false),
        }
    }

    /// 返回 SQLx statement 是否来自当前 Adapter 后端。
    pub(crate) fn matches_backend(&self, sqlite: bool) -> bool {
        match (&self.backend, sqlite) {
            (SqlxPreparedStatementBackend::Sqlite(statement), true) => statement.sql() == self.sql,
            (SqlxPreparedStatementBackend::Any(statement), false) => statement.sql() == self.sql,
            _ => false,
        }
    }

    /// 返回 SQLx Any 的真实预编译语句。
    pub(crate) fn any_statement(&self) -> Option<&AnyStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::Any(statement) => Some(statement),
            SqlxPreparedStatementBackend::Sqlite(_) => None,
        }
    }

    /// 返回 SQLx SQLite 的真实预编译语句。
    pub(crate) fn sqlite_statement(&self) -> Option<&SqliteStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::Sqlite(statement) => Some(statement),
            SqlxPreparedStatementBackend::Any(_) => None,
        }
    }
}

impl PhysicalPreparedStatement for SqlxPreparedStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
