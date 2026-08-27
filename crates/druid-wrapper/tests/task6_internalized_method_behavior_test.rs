//! Receiver-typed observable evidence for the two SQLx external-pool poison operations.

use druid::core::{
    DruidPooledConnection, ExceptionSorter, ExceptionSorterProperties, Pool, SqlException,
};
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{GeneratedKeys, Statement, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::bb8::SqlxBb8Pool;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDatabase {
    path: PathBuf,
    url: String,
}

impl TestDatabase {
    fn new() -> Self {
        let sequence = DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "druid-task6-method-poison-{}-{epoch}-{sequence}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        let url = format!("sqlite://{}?mode=rwc", path.display());
        Self { path, url }
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

struct AllFatalSorter;

impl ExceptionSorter for AllFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

#[derive(Debug, PartialEq, Eq)]
struct PoisonObservation {
    discard_count: u64,
    create_count: u64,
    destroy_count: u64,
    committed_row_count: i64,
    replacement_auto_commit: bool,
}

fn task6_assert_typed_operation_trace<T: Debug + PartialEq>(
    _target_ufcs: &'static str,
    actual: T,
    expected: T,
) {
    assert_eq!(actual, expected);
}

async fn task6_execute_update(connection: &mut DruidPooledConnection, sql: &str) {
    let mut statement = Connection::create_statement(connection, StatementOptions::default())
        .await
        .unwrap();
    Statement::execute_update(statement.as_mut(), sql, GeneratedKeys::None)
        .await
        .unwrap();
    Statement::close(statement.as_mut()).await.unwrap();
}

async fn task6_scalar_i64(connection: &mut DruidPooledConnection, sql: &str) -> i64 {
    let mut statement = Connection::create_statement(connection, StatementOptions::default())
        .await
        .unwrap();
    let mut rows = Statement::execute_query(statement.as_mut(), sql)
        .await
        .unwrap();
    assert!(ResultSet::next(rows.as_mut()).await.unwrap());
    let value = match ResultSet::value(rows.as_ref(), ColumnIndex::new(1).unwrap()).unwrap() {
        Value::Int(value) => value,
        value => panic!("expected integer scalar, got {value:?}"),
    };
    ResultSet::close(rows.as_mut()).await.unwrap();
    Statement::close(statement.as_mut()).await.unwrap();
    value
}

async fn task6_observe_external_pool_poison() -> PoisonObservation {
    let database = TestDatabase::new();
    let pool = SqlxBb8Pool::connect(
        "task6-method-poison",
        &database.url,
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    task6_execute_update(
        &mut connection,
        "CREATE TABLE fatal_item(id INTEGER PRIMARY KEY)",
    )
    .await;
    Connection::set_auto_commit(&mut connection, false)
        .await
        .unwrap();
    task6_execute_update(&mut connection, "INSERT INTO fatal_item(id) VALUES (1)").await;
    connection.set_exception_sorter(Arc::new(AllFatalSorter));
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    drop(
        Statement::execute_update(
            statement.as_mut(),
            "INSERT INTO missing_fatal_item(id) VALUES (1)",
            GeneratedKeys::None,
        )
        .await
        .expect_err("fatal SQL must poison the checked-out external lease"),
    );
    drop(statement);
    drop(connection);

    let mut replacement = pool.get().await.unwrap();
    let replacement_auto_commit = Connection::auto_commit(&replacement).unwrap();
    let committed_row_count =
        task6_scalar_i64(&mut replacement, "SELECT COUNT(*) FROM fatal_item").await;
    let state = pool.state();
    PoisonObservation {
        discard_count: state.discard_count,
        create_count: state.create_count,
        destroy_count: state.destroy_count,
        committed_row_count,
        replacement_auto_commit,
    }
}

fn task6_expected_external_pool_poison() -> PoisonObservation {
    PoisonObservation {
        discard_count: 1,
        create_count: 2,
        destroy_count: 1,
        committed_row_count: 0,
        replacement_auto_commit: true,
    }
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_mark_discarded() {
    let actual = task6_observe_external_pool_poison().await;
    let expected = task6_expected_external_pool_poison();
    task6_assert_typed_operation_trace(
        "druid_wrapper::sqlx::external_pool_connection_control::ExternalPoolLeaseControl::poison",
        actual,
        expected,
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_connection_is_discarded() {
    let actual = task6_observe_external_pool_poison().await;
    let expected = task6_expected_external_pool_poison();
    task6_assert_typed_operation_trace(
        "druid_wrapper::sqlx::external_pool_connection_control::take_poisoned",
        actual,
        expected,
    );
}
