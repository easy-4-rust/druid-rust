//! Canonical SQLx deadpool external-pool contracts.

use druid::core::{
    BeforeFilter, ConnectionEvent, DruidError, DruidPooledConnection, ExceptionSorter,
    ExceptionSorterProperties, ExecContext, FilterChain, Pool, SqlException,
};
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, Statement, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::deadpool::SqlxDeadpoolPool;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

static DATABASE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct TestDatabase {
    path: PathBuf,
    url: String,
}

impl TestDatabase {
    fn new(label: &str) -> Self {
        let sequence = DATABASE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "druid-task6-deadpool-{label}-{}-{epoch}-{sequence}.sqlite",
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

struct RollbackRejectingFilter;

#[async_trait::async_trait]
impl BeforeFilter for RollbackRejectingFilter {
    fn name(&self) -> &str {
        "task6-reject-rollback"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    async fn on_connection_event(&self, event: &ConnectionEvent) -> Result<(), DruidError> {
        if matches!(event, ConnectionEvent::Rollback) {
            Err(DruidError::Other(
                "task6 forced rollback recycle failure".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

fn rollback_rejecting_chain() -> Arc<FilterChain> {
    let mut chain = FilterChain::new();
    chain.add_before(Arc::new(RollbackRejectingFilter));
    Arc::new(chain)
}

async fn execute_update(connection: &mut DruidPooledConnection, sql: &str) {
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update(sql, GeneratedKeys::None)
        .await
        .unwrap();
    Statement::close(statement.as_mut()).await.unwrap();
}

async fn scalar_i64(connection: &mut DruidPooledConnection, sql: &str) -> i64 {
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement.execute_query(sql).await.unwrap();
    assert!(rows.next().await.unwrap());
    let value = match rows.value(ColumnIndex::new(1).unwrap()).unwrap() {
        Value::Int(value) => value,
        value => panic!("expected integer scalar, got {value:?}"),
    };
    rows.close().await.unwrap();
    Statement::close(statement.as_mut()).await.unwrap();
    value
}

#[tokio::test]
async fn deadpool_executes_recycles_and_resets_one_canonical_lease() {
    let pool = SqlxDeadpoolPool::connect(
        "deadpool-canonical",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update("CREATE TABLE item(id INTEGER)", GeneratedKeys::None)
        .await
        .unwrap();
    statement.close().await.unwrap();
    connection.set_auto_commit(false).await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(pool.state().recycle_count, 1);
    let reused = pool.get().await.unwrap();
    assert!(reused.auto_commit().unwrap());
    let state = pool.state();
    assert_eq!(state.create_count, 1);
    assert_eq!(state.destroy_count, 0);
}

#[tokio::test]
async fn deadpool_timeout_close_and_invalid_capacity_are_explicit() {
    assert!(SqlxDeadpoolPool::connect(
        "invalid",
        "sqlite::memory:",
        0,
        Duration::from_secs(1),
        None,
    )
    .is_err());
    let pool = SqlxDeadpoolPool::connect(
        "timeout",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .unwrap();
    let _held = pool.get().await.unwrap();
    assert_eq!(
        pool.get_timeout(Duration::from_millis(1))
            .await
            .unwrap_err(),
        druid_rdbc::error::RdbcError::AcquireTimeout
    );
    pool.close();
    assert!(pool.state().closed);
}

#[tokio::test]
async fn deadpool_dirty_transaction_drop_is_discarded() {
    let database = TestDatabase::new("dirty-drop");
    let pool = SqlxDeadpoolPool::connect(
        "deadpool-dirty",
        &database.url,
        1,
        Duration::from_secs(1),
        None,
    )
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    execute_update(
        &mut connection,
        "CREATE TABLE dirty_item(id INTEGER PRIMARY KEY)",
    )
    .await;
    connection.set_auto_commit(false).await.unwrap();
    execute_update(&mut connection, "INSERT INTO dirty_item(id) VALUES (1)").await;
    drop(connection);
    assert_eq!(pool.state().discard_count, 1);

    let mut replacement = pool.get().await.unwrap();
    assert!(replacement.auto_commit().unwrap());
    assert_eq!(
        scalar_i64(&mut replacement, "SELECT COUNT(*) FROM dirty_item").await,
        0
    );
    let state = pool.state();
    assert_eq!(state.create_count, 2);
    assert_eq!(state.destroy_count, 1);
}

#[tokio::test]
async fn deadpool_fatal_disposition_poison_rejects_transaction_dirty_connection() {
    let database = TestDatabase::new("fatal");
    let pool = SqlxDeadpoolPool::connect(
        "deadpool-fatal",
        &database.url,
        1,
        Duration::from_secs(1),
        None,
    )
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    execute_update(
        &mut connection,
        "CREATE TABLE fatal_item(id INTEGER PRIMARY KEY)",
    )
    .await;
    connection.set_auto_commit(false).await.unwrap();
    execute_update(&mut connection, "INSERT INTO fatal_item(id) VALUES (1)").await;
    connection.set_exception_sorter(Arc::new(AllFatalSorter));
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    assert!(statement
        .execute_update(
            "INSERT INTO missing_fatal_item(id) VALUES (1)",
            GeneratedKeys::None
        )
        .await
        .is_err());
    drop(statement);
    drop(connection);
    assert_eq!(pool.state().discard_count, 1);

    let mut replacement = pool.get().await.unwrap();
    assert!(replacement.auto_commit().unwrap());
    assert_eq!(
        scalar_i64(&mut replacement, "SELECT COUNT(*) FROM fatal_item").await,
        0
    );
    let state = pool.state();
    assert_eq!(state.create_count, 2);
    assert_eq!(state.destroy_count, 1);
}

#[tokio::test]
async fn deadpool_recycle_error_poison_rejects_transaction_dirty_connection() {
    let database = TestDatabase::new("recycle-error");
    let pool = SqlxDeadpoolPool::connect(
        "deadpool-recycle-error",
        &database.url,
        1,
        Duration::from_secs(1),
        Some(rollback_rejecting_chain()),
    )
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    execute_update(
        &mut connection,
        "CREATE TABLE recycle_item(id INTEGER PRIMARY KEY)",
    )
    .await;
    connection.set_auto_commit(false).await.unwrap();
    execute_update(&mut connection, "INSERT INTO recycle_item(id) VALUES (1)").await;
    connection.close().await.unwrap();
    assert_eq!(pool.state().discard_count, 1);
    assert_eq!(pool.state().recycle_error_count, 1);

    let mut replacement = pool.get().await.unwrap();
    assert!(replacement.auto_commit().unwrap());
    assert_eq!(
        scalar_i64(&mut replacement, "SELECT COUNT(*) FROM recycle_item").await,
        0
    );
    let state = pool.state();
    assert_eq!(state.create_count, 2);
    assert_eq!(state.destroy_count, 1);
}
