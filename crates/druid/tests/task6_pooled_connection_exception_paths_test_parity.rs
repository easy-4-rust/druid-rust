//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Canonical pooled Connection exception classification contract.
use druid::core::{
    Connection, ConnectionRecycleDisposition, DruidConnectionHolder, DruidPooledConnection,
    ExceptionSorter, ExceptionSorterProperties, ExecResult, RdbcError, Row, Savepoint,
    SqlException, Value,
};
use druid_rdbc::connection::TransactionIsolation;
use druid_rdbc::statement::{ResultSetHoldability, StatementOptions};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct AlwaysFatalSorter;
impl ExceptionSorter for AlwaysFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

struct FailingConnection;
impl FailingConnection {
    fn error<T>() -> Result<T, RdbcError> {
        Err(RdbcError::SqlException(Box::new(
            SqlException::driver(17002, "Io exception: Connection reset")
                .with_sql_state("08006")
                .with_class_name("java.sql.SQLRecoverableException"),
        )))
    }
}
druid_rdbc::impl_standard_wrapper!(FailingConnection => Connection);

#[async_trait::async_trait]
impl Connection for FailingConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Self::error()
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Self::error()
    }
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Self::error()
    }
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        Self::error()
    }
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::CallableStatement>, RdbcError> {
        Self::error()
    }
    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        Self::error()
    }
    async fn begin(&mut self) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn commit(&mut self) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn rollback(&mut self) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn rollback_to(&mut self, _savepoint: &Savepoint) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        Self::error()
    }
    async fn set_savepoint_named(&mut self, _name: &str) -> Result<Savepoint, RdbcError> {
        Self::error()
    }
    async fn release_savepoint(&mut self, _savepoint: &Savepoint) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn abort(&mut self) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn ping(&mut self) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(true)
    }
    async fn set_auto_commit(&mut self, _auto_commit: bool) -> Result<(), RdbcError> {
        Self::error()
    }
    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    async fn set_read_only(&mut self, _read_only: bool) -> Result<(), RdbcError> {
        Self::error()
    }
    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        Ok(TransactionIsolation::READ_COMMITTED)
    }
    async fn set_transaction_isolation(
        &mut self,
        _level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        Self::error()
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Ok(ResultSetHoldability::CloseAtCommit)
    }
    async fn set_holdability(
        &mut self,
        _holdability: ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn warnings(&mut self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        Ok(None)
    }
    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        Self::error()
    }
    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        Ok(None)
    }
    async fn set_catalog(&mut self, _catalog: Option<&str>) -> Result<(), RdbcError> {
        Self::error()
    }
    fn schema(&self) -> Result<Option<String>, RdbcError> {
        Ok(None)
    }
    async fn set_schema(&mut self, _schema: Option<&str>) -> Result<(), RdbcError> {
        Self::error()
    }
    async fn create_blob(&mut self) -> Result<druid_rdbc::types::RdbcBlob, RdbcError> {
        Self::error()
    }
    async fn create_clob(&mut self) -> Result<druid_rdbc::types::RdbcClob, RdbcError> {
        Self::error()
    }
    async fn create_n_clob(&mut self) -> Result<druid_rdbc::types::RdbcNClob, RdbcError> {
        Self::error()
    }
    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("failing".to_owned())
    }
}

async fn task6_failing_connection() -> (DruidPooledConnection, Arc<AtomicBool>) {
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(FailingConnection),
        1,
        Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let discarded = Arc::new(AtomicBool::new(false));
    let callback_discarded = Arc::clone(&discarded);
    let pooled = DruidPooledConnection::with_standard_holder_and_disposition_observer(
        holder,
        "exception-paths".to_owned(),
        None,
        false,
        false,
        Box::new(move |disposition| {
            callback_discarded.store(
                matches!(disposition, ConnectionRecycleDisposition::Discard { .. }),
                Ordering::Release,
            );
        }),
        Box::new(|_, _| false),
    )
    .with_exception_sorter(Arc::new(AlwaysFatalSorter));
    (pooled, discarded)
}

fn task6_assert_fatal<T>(result: Result<T, RdbcError>, discarded: &AtomicBool, operation: &str) {
    assert!(
        matches!(result, Err(RdbcError::SqlException(_))),
        "{operation}"
    );
    assert!(
        discarded.load(Ordering::Acquire),
        "{operation} must synchronously discard"
    );
}

#[tokio::test]
async fn task6_pooled_connection_exception_paths_test_every_standard_connection_error_path_invokes_exception_sorter(
) {
    let savepoint = Savepoint {
        id: 1,
        name: Some("sp_1".to_owned()),
    };

    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.exec("UPDATE t SET v = 1", Vec::new()).await, &d, "exec");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.fetch("SELECT 1", Vec::new()).await, &d, "fetch");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(
        c.create_statement(StatementOptions::default()).await,
        &d,
        "create_statement",
    );
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(
        c.prepare_statement("SELECT 1", StatementOptions::default())
            .await,
        &d,
        "prepare_statement",
    );
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(
        c.prepare_call("CALL p()", StatementOptions::default())
            .await,
        &d,
        "prepare_call",
    );
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.database_meta_data().await, &d, "database_meta_data");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.begin().await, &d, "begin");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.commit().await, &d, "commit");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.rollback().await, &d, "rollback");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.rollback_to(&savepoint).await, &d, "rollback_to");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.set_savepoint().await, &d, "set_savepoint");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(
        c.set_savepoint_named("sp_1").await,
        &d,
        "set_savepoint_named",
    );
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(
        c.release_savepoint(&savepoint).await,
        &d,
        "release_savepoint",
    );
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.abort().await, &d, "abort");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.ping().await, &d, "ping");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.set_auto_commit(false).await, &d, "set_auto_commit");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.set_read_only(true).await, &d, "set_read_only");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(
        c.set_transaction_isolation(TransactionIsolation::SERIALIZABLE)
            .await,
        &d,
        "set_transaction_isolation",
    );
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(
        c.set_holdability(ResultSetHoldability::HoldOverCommit)
            .await,
        &d,
        "set_holdability",
    );
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.clear_warnings().await, &d, "clear_warnings");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.set_catalog(Some("catalog")).await, &d, "set_catalog");
    let (mut c, d) = task6_failing_connection().await;
    task6_assert_fatal(c.set_schema(Some("schema")).await, &d, "set_schema");
}
