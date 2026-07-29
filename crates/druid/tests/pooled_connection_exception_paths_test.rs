//! `DruidPooledConnection` 全连接操作异常分类合同。
//!
//! 对应 Java：
//! `com.alibaba.druid.bvt.pool.exception.OracleExceptionSorterTest_*`。

use druid::core::{
    DruidError, DruidPooledConnection, ExceptionSorter, ExceptionSorterProperties, ExecResult,
    PhysicalConnection, PhysicalPreparedStatement, PreparedStatementKey,
    PreparedStatementMethodType, Row, Savepoint, SqlException, SqlTextPreparedStatement,
    StatementExecuteResult, StatementGeneratedKeys, Value,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct AlwaysFatalSorter;

impl ExceptionSorter for AlwaysFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

struct FailingPhysicalConnection {
    discarded: Arc<AtomicBool>,
}

impl FailingPhysicalConnection {
    fn error<T>() -> Result<T, DruidError> {
        Err(DruidError::SqlException(Box::new(
            SqlException::driver(17002, "Io exception: Connection reset")
                .with_sql_state("08006")
                .with_class_name("java.sql.SQLRecoverableException"),
        )))
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for FailingPhysicalConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Self::error()
    }

    async fn execute(
        &mut self,
        _sql: &str,
        _params: Vec<Value>,
        _generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        Self::error()
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Self::error()
    }

    async fn prepare_physical_statement(
        &mut self,
        _key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        Self::error()
    }

    async fn prepare_physical_call(
        &mut self,
        _key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        Self::error()
    }

    async fn exec_prepared(
        &mut self,
        _statement: &dyn PhysicalPreparedStatement,
        _params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        Self::error()
    }

    async fn fetch_prepared(
        &mut self,
        _statement: &dyn PhysicalPreparedStatement,
        _params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        Self::error()
    }

    async fn close_prepared_statement(
        &mut self,
        _statement: Arc<dyn PhysicalPreparedStatement>,
    ) -> Result<(), DruidError> {
        Self::error()
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Self::error()
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Self::error()
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Self::error()
    }

    async fn rollback_to(&mut self, _savepoint: &Savepoint) -> Result<(), DruidError> {
        Self::error()
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        Self::error()
    }

    async fn set_savepoint_named(&mut self, _name: &str) -> Result<Savepoint, DruidError> {
        Self::error()
    }

    async fn release_savepoint(&mut self, _savepoint: &Savepoint) -> Result<(), DruidError> {
        Self::error()
    }

    async fn abort(&mut self) -> Result<(), DruidError> {
        Self::error()
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        Self::error()
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn set_auto_commit(&mut self, _auto_commit: bool) -> Result<(), DruidError> {
        Self::error()
    }

    async fn set_read_only(&mut self, _read_only: bool) -> Result<(), DruidError> {
        Self::error()
    }

    async fn set_transaction_isolation(&mut self, _level: u8) -> Result<(), DruidError> {
        Self::error()
    }

    async fn set_holdability(&mut self, _holdability: i32) -> Result<(), DruidError> {
        Self::error()
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        Self::error()
    }

    fn mark_discarded(&mut self) {
        self.discarded.store(true, Ordering::Release);
    }

    fn is_discarded(&self) -> bool {
        self.discarded.load(Ordering::Acquire)
    }

    async fn set_catalog(&mut self, _catalog: &str) -> Result<(), DruidError> {
        Self::error()
    }

    async fn set_schema(&mut self, _schema: &str) -> Result<(), DruidError> {
        Self::error()
    }
}

fn failing_connection() -> (DruidPooledConnection, Arc<AtomicBool>) {
    let discarded = Arc::new(AtomicBool::new(false));
    let connection = FailingPhysicalConnection {
        discarded: Arc::clone(&discarded),
    };
    let pooled = DruidPooledConnection::new(Box::new(connection), 1, Box::new(|_, _| {}))
        .with_exception_sorter(Arc::new(AlwaysFatalSorter));
    (pooled, discarded)
}

fn assert_fatal<T>(result: Result<T, DruidError>, discarded: &AtomicBool, operation: &str) {
    assert!(
        matches!(result, Err(DruidError::SqlException(_))),
        "{operation} 必须原样返回结构化 SQL 异常"
    );
    assert!(
        discarded.load(Ordering::Acquire),
        "{operation} 的 fatal SQL 异常必须同步标记物理连接为 discard"
    );
}

#[tokio::test]
async fn every_physical_connection_error_path_invokes_exception_sorter() {
    let key = PreparedStatementKey::new(
        Some("select 1".to_string()),
        None,
        PreparedStatementMethodType::M1,
    )
    .expect("prepared statement key");
    let savepoint = Savepoint {
        id: 1,
        name: Some("sp_1".to_string()),
    };

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.exec("update t set v = 1", vec![]).await,
        &discarded,
        "exec",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.fetch("select 1", vec![]).await,
        &discarded,
        "fetch",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection
            .execute("select 1", vec![], StatementGeneratedKeys::None)
            .await,
        &discarded,
        "execute",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.prepare_physical_statement(&key).await,
        &discarded,
        "prepare_physical_statement",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.prepare_physical_call(&key).await,
        &discarded,
        "prepare_physical_call",
    );

    let statement = Arc::new(SqlTextPreparedStatement::new("select 1"));
    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.exec_prepared(statement.as_ref(), vec![]).await,
        &discarded,
        "exec_prepared",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.fetch_prepared(statement.as_ref(), vec![]).await,
        &discarded,
        "fetch_prepared",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.close_prepared_statement(statement).await,
        &discarded,
        "close_prepared_statement",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(connection.begin().await, &discarded, "begin");

    let (mut connection, discarded) = failing_connection();
    assert_fatal(connection.commit().await, &discarded, "commit");

    let (mut connection, discarded) = failing_connection();
    assert_fatal(connection.rollback().await, &discarded, "rollback");

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.rollback_to(&savepoint).await,
        &discarded,
        "rollback_to",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.set_savepoint().await,
        &discarded,
        "set_savepoint",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.set_savepoint_named("sp_1").await,
        &discarded,
        "set_savepoint_named",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.release_savepoint(&savepoint).await,
        &discarded,
        "release_savepoint",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(connection.abort().await, &discarded, "abort");

    let (mut connection, discarded) = failing_connection();
    assert_fatal(connection.ping().await, &discarded, "ping");

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.set_auto_commit(false).await,
        &discarded,
        "set_auto_commit",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.set_read_only(true).await,
        &discarded,
        "set_read_only",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.set_transaction_isolation(8).await,
        &discarded,
        "set_transaction_isolation",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.set_holdability(1).await,
        &discarded,
        "set_holdability",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.clear_warnings().await,
        &discarded,
        "clear_warnings",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.set_catalog("catalog").await,
        &discarded,
        "set_catalog",
    );

    let (mut connection, discarded) = failing_connection();
    assert_fatal(
        connection.set_schema("schema").await,
        &discarded,
        "set_schema",
    );
}
