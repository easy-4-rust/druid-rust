//! Java Druid 连接回收语义契约测试。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 对照对象：
//!
//! - `DruidPooledConnection#close()`
//! - `DruidDataSource#recycle(DruidPooledConnection)`
//! - `DruidConnectionHolder#reset()`
//!
//! 对照测试：
//!
//! - `DruidDataSourceTest_recycle`
//! - `DruidDataSourceTest_recycle2`
//! - `DruidDataSourceTest9_phyMaxUseCount`
//! - `TestDefault#test_close`

use druid::core::{DruidError, ValidConnectionChecker};
use druid::pool::DruidPool;
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::extensions::ConnectionCapabilities;
use druid_rdbc::statement::{ExecResult, ResultSetHoldability, Statement, StatementOptions};
use druid_rdbc::types::{Row, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[path = "task6_recycle_fixture/tracking_validation_statement.rs"]
mod tracking_validation_statement;

#[derive(Debug)]
struct ConnectionState {
    auto_commit: bool,
    read_only: bool,
    holdability: i32,
    transaction_isolation: i32,
    catalog: Option<String>,
    closed: bool,
    fail_operation: Option<&'static str>,
    events: Vec<&'static str>,
    close_calls: usize,
    close_gate: Option<Arc<tokio::sync::Semaphore>>,
    close_started: Option<Arc<tokio::sync::Notify>>,
    close_completed: Option<Arc<tokio::sync::Notify>>,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self {
            auto_commit: true,
            read_only: false,
            holdability: 0,
            transaction_isolation: 2,
            catalog: None,
            closed: false,
            fail_operation: None,
            events: Vec::new(),
            close_calls: 0,
            close_gate: None,
            close_started: None,
            close_completed: None,
        }
    }
}

struct TrackingConnection {
    state: Arc<Mutex<ConnectionState>>,
}

impl TrackingConnection {
    fn operation(&self, operation: &'static str) -> Result<(), RdbcError> {
        let mut state = self.state.lock().expect("tracking state poisoned");
        state.events.push(operation);
        if state.fail_operation == Some(operation) {
            Err(RdbcError::DriverError(format!("{operation} failed")))
        } else {
            Ok(())
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingConnection => Connection);

#[async_trait::async_trait]
impl Connection for TrackingConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Ok(Box::new(
            tracking_validation_statement::TrackingValidationStatement::new(TrackingConnection {
                state: Arc::clone(&self.state),
            }),
        ))
    }

    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult::default())
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Ok(vec![Row::new(vec![Value::Int(1)])])
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        self.operation("begin")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.operation("commit")
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        // RDBC rollback 不改变 autoCommit，随后 holder.reset 再恢复默认值。
        self.operation("rollback")
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        self.operation("ping")
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        let (gate, started) = {
            let mut state = self.state.lock().expect("tracking state poisoned");
            state.close_calls += 1;
            (state.close_gate.clone(), state.close_started.clone())
        };
        if let Some(started) = started {
            started.notify_one();
        }
        if let Some(gate) = gate {
            gate.acquire()
                .await
                .expect("close gate remains open")
                .forget();
        }
        let mut state = self.state.lock().expect("tracking state poisoned");
        if !state.closed {
            state.events.push("close");
            state.closed = true;
        }
        if let Some(completed) = &state.close_completed {
            completed.notify_one();
        }
        if state.fail_operation == Some("close") {
            Err(RdbcError::DriverError("close failed".to_owned()))
        } else {
            Ok(())
        }
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.state.lock().expect("tracking state poisoned").closed)
    }

    fn capabilities(&self) -> ConnectionCapabilities {
        ConnectionCapabilities {
            transactions: true,
            savepoints: false,
            catalogs: false,
            schemas: false,
            callable_statements: false,
        }
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(self
            .state
            .lock()
            .expect("tracking state poisoned")
            .auto_commit)
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), RdbcError> {
        self.operation("set_auto_commit")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .auto_commit = auto_commit;
        Ok(())
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(self
            .state
            .lock()
            .expect("tracking state poisoned")
            .read_only)
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        self.operation("set_read_only")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .read_only = read_only;
        Ok(())
    }

    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        Ok(TransactionIsolation::from_jdbc(
            self.state
                .lock()
                .expect("tracking state poisoned")
                .transaction_isolation,
        ))
    }

    async fn set_transaction_isolation(
        &mut self,
        level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.operation("set_transaction_isolation")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .transaction_isolation = level.jdbc_code();
        Ok(())
    }

    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Ok(ResultSetHoldability::DriverValue(
            self.state
                .lock()
                .expect("tracking state poisoned")
                .holdability,
        ))
    }

    async fn set_holdability(
        &mut self,
        holdability: ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        self.operation("set_holdability")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .holdability = holdability.rdbc_code();
        Ok(())
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.operation("clear_warnings")
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        self.operation("set_catalog")?;
        self.state.lock().expect("tracking state poisoned").catalog = catalog.map(str::to_owned);
        Ok(())
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("tracking".to_owned())
    }
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        Ok(None)
    }
}

#[derive(Default)]
struct TrackingFactory {
    sequence: AtomicU64,
    states: Mutex<Vec<Arc<Mutex<ConnectionState>>>>,
    validation_count: Arc<AtomicUsize>,
    validation_succeeds: Arc<AtomicBool>,
    initial_state: Mutex<Option<ConnectionState>>,
}

struct TrackingValidConnectionChecker {
    validation_count: Arc<AtomicUsize>,
    validation_succeeds: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl ValidConnectionChecker for TrackingValidConnectionChecker {
    async fn is_valid_connection(
        &self,
        connection: &mut Box<dyn Connection>,
        _query: Option<&str>,
        _validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        self.validation_count.fetch_add(1, Ordering::Relaxed);
        if self.validation_succeeds.load(Ordering::Relaxed) {
            connection.ping().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl TrackingFactory {
    fn new() -> Self {
        Self {
            validation_succeeds: Arc::new(AtomicBool::new(true)),
            ..Self::default()
        }
    }

    fn state(&self, index: usize) -> Arc<Mutex<ConnectionState>> {
        self.states.lock().expect("factory states poisoned")[index].clone()
    }

    fn created_count(&self) -> u64 {
        self.sequence.load(Ordering::Relaxed)
    }

    fn set_initial_state(&self, initial_state: ConnectionState) {
        *self.initial_state.lock().expect("initial state poisoned") = Some(initial_state);
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for TrackingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.sequence.fetch_add(1, Ordering::Relaxed);
        let initial_state = self
            .initial_state
            .lock()
            .expect("initial state poisoned")
            .take()
            .unwrap_or_default();
        let state = Arc::new(Mutex::new(initial_state));
        self.states
            .lock()
            .expect("factory states poisoned")
            .push(state.clone());
        Ok(Box::new(TrackingConnection { state }))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        self.validation_count.fetch_add(1, Ordering::Relaxed);
        if self.validation_succeeds.load(Ordering::Relaxed) {
            connection.ping().await
        } else {
            Err(RdbcError::ValidationFailed(
                "tracking validation failed".to_string(),
            ))
        }
    }
}

async fn build_pool(
    factory: Arc<TrackingFactory>,
    configure: impl FnOnce(druid::pool::DruidPoolBuilder) -> druid::pool::DruidPoolBuilder,
) -> DruidPool {
    configure(
        DruidPool::builder()
            .name("recycle-contract")
            .driver_name("tracking")
            .factory(factory)
            .max_open(2)
            .max_idle(2)
            .max_lifetime(Duration::from_mins(1))
            .acquire_timeout(Duration::from_secs(1)),
    )
    .build()
    .await
    .expect("tracking pool must build")
}

#[tokio::test]
async fn task6_recycle_complete_explicit_close_rolls_back_and_resets_in_java_order() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder).await;
    let mut connection = pool.get().await.expect("connection must open");
    let connection_id = connection.id();
    connection.begin().await.expect("transaction must begin");
    connection
        .set_holdability(ResultSetHoldability::HoldOverCommit)
        .await
        .expect("holdability must change");
    connection
        .set_transaction_isolation(TransactionIsolation::SERIALIZABLE)
        .await
        .expect("isolation must change");

    let state = factory.state(0);
    state
        .lock()
        .expect("tracking state poisoned")
        .events
        .clear();
    connection
        .close()
        .await
        .expect("recycle errors are swallowed");

    assert_eq!(
        state.lock().expect("tracking state poisoned").events,
        vec![
            "rollback",
            "set_holdability",
            "set_transaction_isolation",
            "set_auto_commit",
            "clear_warnings"
        ]
    );
    let pool_state = pool.state();
    assert_eq!(pool_state.active_count, 0);
    assert_eq!(pool_state.idle_count, 1);
    assert_eq!(pool_state.close_count, 1);
    assert_eq!(pool_state.recycle_count, 1);
    assert_eq!(pool_state.destroy_count, 0);

    let connection = pool.get().await.expect("connection must be reusable");
    assert_eq!(connection.id(), connection_id);
    assert!(connection.auto_commit().unwrap());
    assert!(!connection.read_only().unwrap());
    assert_eq!(connection.holdability().unwrap().rdbc_code(), 0);
    assert_eq!(connection.transaction_isolation().unwrap().jdbc_code(), 2);
}

#[tokio::test]
async fn task6_recycle_complete_read_only_transaction_skips_rollback_then_restores_defaults() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder).await;
    let mut connection = pool.get().await.expect("connection must open");
    connection.begin().await.expect("transaction must begin");
    connection
        .set_read_only(true)
        .await
        .expect("read-only must change");
    let state = factory.state(0);
    state
        .lock()
        .expect("tracking state poisoned")
        .events
        .clear();

    connection.close().await.expect("connection must recycle");

    assert_eq!(
        state.lock().expect("tracking state poisoned").events,
        vec!["set_read_only", "set_auto_commit", "clear_warnings"]
    );
    assert_eq!(pool.state().idle_count, 1);
}

#[tokio::test]
async fn task6_recycle_complete_rollback_failure_is_swallowed_counted_and_connection_discarded() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder).await;
    let mut connection = pool.get().await.expect("connection must open");
    let first_id = connection.id();
    connection.begin().await.expect("transaction must begin");
    let state = factory.state(0);
    {
        let mut state = state.lock().expect("tracking state poisoned");
        state.events.clear();
        state.fail_operation = Some("rollback");
    }

    connection
        .close()
        .await
        .expect("Java recycle swallows rollback failure");

    let pool_state = pool.state();
    assert_eq!(pool_state.active_count, 0);
    assert_eq!(pool_state.idle_count, 0);
    assert_eq!(pool_state.close_count, 1);
    assert_eq!(pool_state.recycle_count, 0);
    assert_eq!(pool_state.recycle_error_count, 1);
    assert_eq!(pool_state.discard_count, 1);
    assert_eq!(pool_state.destroy_count, 1);
    assert!(state.lock().expect("tracking state poisoned").closed);

    let replacement = pool.get().await.expect("replacement must be created");
    assert_ne!(replacement.id(), first_id);
    assert_eq!(factory.created_count(), 2);
}

#[tokio::test]
async fn task6_recycle_complete_test_on_return_validates_and_invalid_connection_is_not_recycled() {
    let factory = Arc::new(TrackingFactory::new());
    let checker = Arc::new(TrackingValidConnectionChecker {
        validation_count: Arc::clone(&factory.validation_count),
        validation_succeeds: Arc::clone(&factory.validation_succeeds),
    });
    let pool = build_pool(factory.clone(), |builder| {
        builder
            .test_on_return(true)
            .valid_connection_checker(checker)
    })
    .await;

    let mut first = pool.get().await.expect("connection must open");
    first.close().await.expect("valid connection must recycle");
    // 一次物理创建校验 + 一次 testOnReturn 校验。
    assert_eq!(factory.validation_count.load(Ordering::Relaxed), 2);
    assert_eq!(pool.state().recycle_count, 1);

    let mut second = pool.get().await.expect("idle connection must open");
    factory.validation_succeeds.store(false, Ordering::Relaxed);
    second
        .close()
        .await
        .expect("validation failure is swallowed on close");

    let pool_state = pool.state();
    assert_eq!(factory.validation_count.load(Ordering::Relaxed), 3);
    assert_eq!(pool_state.idle_count, 0);
    assert_eq!(pool_state.recycle_count, 1);
    assert_eq!(pool_state.recycle_error_count, 0);
    assert_eq!(pool_state.discard_count, 1);
    assert_eq!(pool_state.destroy_count, 1);
}

#[tokio::test]
async fn task6_recycle_complete_physical_connection_is_discarded_at_configured_max_use_count() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder.max_use_count(2)).await;

    let mut first = pool.get().await.expect("first lease must open");
    let first_id = first.id();
    first.close().await.expect("first lease must recycle");

    let mut second = pool.get().await.expect("second lease must open");
    assert_eq!(second.id(), first_id);
    second
        .close()
        .await
        .expect("second lease close must succeed");

    let pool_state = pool.state();
    assert_eq!(pool_state.recycle_count, 1);
    assert_eq!(pool_state.discard_count, 1);
    assert_eq!(pool_state.destroy_count, 1);

    let replacement = pool.get().await.expect("replacement must open");
    assert_ne!(replacement.id(), first_id);
    assert_eq!(factory.created_count(), 2);
}

#[tokio::test]
async fn task6_recycle_complete_dirty_drop_discards_instead_of_leaking_transaction_to_idle_queue() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder).await;
    let mut connection = pool.get().await.expect("connection must open");
    connection.begin().await.expect("transaction must begin");
    let state = factory.state(0);
    state
        .lock()
        .expect("tracking state poisoned")
        .events
        .clear();

    drop(connection);

    let pool_state = pool.state();
    assert_eq!(pool_state.active_count, 0);
    assert_eq!(pool_state.idle_count, 0);
    assert_eq!(pool_state.recycle_count, 0);
    assert_eq!(pool_state.discard_count, 1);
    assert_eq!(pool_state.destroy_count, 1);
    assert!(
        !state
            .lock()
            .expect("tracking state poisoned")
            .events
            .contains(&"rollback"),
        "Drop cannot await rollback and therefore must discard"
    );
}

#[tokio::test]
async fn task6_recycle_complete_configured_isolation_preservation_skips_reset() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| {
        builder.keep_connection_underlying_transaction_isolation(true)
    })
    .await;
    let mut connection = pool.get().await.expect("connection must open");
    let first_id = connection.id();
    connection
        .set_transaction_isolation(TransactionIsolation::SERIALIZABLE)
        .await
        .expect("isolation must change");
    let state = factory.state(0);
    state
        .lock()
        .expect("tracking state poisoned")
        .events
        .clear();

    connection.close().await.expect("connection must recycle");
    assert_eq!(
        state.lock().expect("tracking state poisoned").events,
        vec!["clear_warnings"]
    );

    let connection = pool.get().await.expect("same connection must be reused");
    assert_eq!(connection.id(), first_id);
    assert_eq!(connection.transaction_isolation().unwrap().jdbc_code(), 8);
}

#[tokio::test]
async fn task6_recycle_complete_physical_connection_defaults_are_initialized_in_java_order() {
    let factory = Arc::new(TrackingFactory::new());
    factory.set_initial_state(ConnectionState {
        auto_commit: false,
        ..ConnectionState::default()
    });
    let pool = build_pool(factory.clone(), |builder| {
        builder
            .default_auto_commit(true)
            .default_read_only(true)
            .default_transaction_isolation(8)
            .default_catalog("tenant_catalog")
    })
    .await;

    let connection = pool.get().await.expect("initialized connection must open");
    assert!(connection.auto_commit().unwrap());
    assert!(connection.read_only().unwrap());
    assert_eq!(connection.transaction_isolation().unwrap().jdbc_code(), 8);
    let state = factory.state(0);
    let state = state.lock().expect("tracking state poisoned");
    assert_eq!(
        state.events,
        vec![
            "set_auto_commit",
            "set_read_only",
            "set_transaction_isolation",
            "set_catalog"
        ]
    );
    assert_eq!(state.catalog.as_deref(), Some("tenant_catalog"));
}

#[tokio::test]
async fn task6_recycle_complete_canonical_holder_moves_through_idle_queue_without_losing_java_metadata(
) {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory, |builder| builder).await;

    let mut first = pool.get().await.unwrap();
    let first_id = first.id();
    {
        let first_holder = first.connection_holder().unwrap();
        assert_eq!(first_holder.connection_id(), first_id);
        assert_eq!(first_holder.use_count(), 1);
        assert_eq!(first_holder.user_password_version(), 0);
    }
    first.exec("SELECT 1", Vec::new()).await.unwrap();
    assert!(first.connection_holder().unwrap().last_exec_idle_duration() < Duration::from_secs(1));
    first.close().await.unwrap();

    let second = pool.get().await.unwrap();
    let second_holder = second.connection_holder().unwrap();
    assert_eq!(second.id(), first_id);
    assert_eq!(second_holder.connection_id(), first_id);
    assert_eq!(second_holder.use_count(), 2);
    assert!(second_holder.idle_duration() < Duration::from_secs(1));
    drop(second);
}

#[tokio::test]
async fn task6_recycle_complete_mysql_family_db_type_enables_java_init_schema_restore_policy() {
    let mysql_factory = Arc::new(TrackingFactory::new());
    let mysql_pool = build_pool(mysql_factory, |builder| builder.db_type_name("mysql")).await;
    let mysql_connection = mysql_pool.get().await.unwrap();
    assert!(mysql_connection
        .connection_holder()
        .unwrap()
        .should_restore_schema_on_recycle());

    let postgres_factory = Arc::new(TrackingFactory::new());
    let postgres_pool = build_pool(postgres_factory, |builder| {
        builder.db_type_name("postgresql")
    })
    .await;
    let postgres_connection = postgres_pool.get().await.unwrap();
    assert!(!postgres_connection
        .connection_holder()
        .unwrap()
        .should_restore_schema_on_recycle());
}

#[tokio::test]
async fn task6_recycle_complete_odps_skips_default_auto_commit_initialization() {
    let factory = Arc::new(TrackingFactory::new());
    factory.set_initial_state(ConnectionState {
        auto_commit: false,
        ..ConnectionState::default()
    });
    let pool = build_pool(factory.clone(), |builder| {
        builder.db_type_name("odps").default_auto_commit(true)
    })
    .await;

    let connection = pool.get().await.expect("ODPS connection must open");
    assert!(!connection.auto_commit().unwrap());
    assert!(!factory
        .state(0)
        .lock()
        .expect("tracking state poisoned")
        .events
        .contains(&"set_auto_commit"));
}

#[tokio::test]
async fn task6_recycle_complete_initialization_failure_closes_connection_and_releases_capacity() {
    let factory = Arc::new(TrackingFactory::new());
    factory.set_initial_state(ConnectionState {
        fail_operation: Some("set_read_only"),
        ..ConnectionState::default()
    });
    let pool = build_pool(factory.clone(), |builder| {
        builder
            .default_read_only(true)
            .initial_size(1)
            .init_exception_throw(true)
    })
    .await;

    let error = pool
        .init()
        .await
        .expect_err("default initialization must fail");
    assert!(matches!(error, DruidError::Rdbc(RdbcError::DriverError(_))));
    let pool_state = pool.state();
    assert_eq!(pool_state.create_count, 1);
    // init 失败属于物理创建阶段，不是一次公共 get 的逻辑获取失败。
    assert_eq!(pool_state.connect_error_count, 0);
    assert_eq!(pool_state.destroy_count, 0);
    assert_eq!(pool_state.active_count, 0);
    assert_eq!(pool_state.idle_count, 0);
    assert!(
        factory
            .state(0)
            .lock()
            .expect("tracking state poisoned")
            .closed
    );
}

/// 显式关闭等待物理关闭；取消等待后再次关闭仍等待同一次 worker 操作。
#[tokio::test]
async fn task6_close_completion_cancel_then_retry_keeps_supervised_owner() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder).await;
    let mut connection = pool.get().await.expect("connection opens");
    connection.begin().await.expect("transaction starts");
    let state = factory.state(0);
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let started = Arc::new(tokio::sync::Notify::new());
    {
        let mut state = state.lock().expect("tracking state poisoned");
        state.fail_operation = Some("rollback");
        state.close_gate = Some(Arc::clone(&gate));
        state.close_started = Some(Arc::clone(&started));
    }
    let mut closing = Box::pin(connection.close());
    tokio::select! {
        result = &mut closing => panic!("logical close completed before physical close: {result:?}"),
        () = started.notified() => {}
        () = tokio::time::sleep(Duration::from_secs(2)) => panic!("close worker did not start"),
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut closing)
            .await
            .is_err()
    );
    drop(closing);
    assert!(!state.lock().expect("tracking state poisoned").closed);
    let counters = pool.state();
    assert_eq!(counters.active_count, 0);
    assert_eq!(counters.destroy_count, 1);
    assert_eq!(counters.discard_count, 1);
    assert_eq!(counters.recycle_error_count, 1);
    let mut retry = Box::pin(connection.close());
    assert!(tokio::time::timeout(Duration::from_millis(10), &mut retry)
        .await
        .is_err());
    gate.add_permits(1);
    tokio::time::timeout(Duration::from_secs(2), retry)
        .await
        .expect("close finishes")
        .expect("recycle error is swallowed");
    connection.close().await.expect("repeated close succeeds");
    drop(connection);
    let state = state.lock().expect("tracking state poisoned");
    assert!(state.closed);
    assert_eq!(state.close_calls, 1);
    assert_eq!(pool.state().close_count, 1);
    assert_eq!(pool.state().destroy_count, 1);
}

/// 池策略淘汰也等待物理关闭；取消并 Drop 不撤销 worker 的唯一所有权。
#[tokio::test]
async fn task6_close_completion_cancel_then_drop_preserves_policy_discard() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder.max_use_count(1)).await;
    let mut connection = pool.get().await.expect("connection opens");
    let state = factory.state(0);
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let started = Arc::new(tokio::sync::Notify::new());
    let completed = Arc::new(tokio::sync::Notify::new());
    {
        let mut state = state.lock().expect("tracking state poisoned");
        state.close_gate = Some(Arc::clone(&gate));
        state.close_started = Some(Arc::clone(&started));
        state.close_completed = Some(Arc::clone(&completed));
    }
    let mut closing = Box::pin(connection.close());
    tokio::select! {
        result = &mut closing => panic!("logical close completed before policy discard: {result:?}"),
        () = started.notified() => {}
        () = tokio::time::sleep(Duration::from_secs(2)) => panic!("close worker did not start"),
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut closing)
            .await
            .is_err()
    );
    drop(closing);
    drop(connection);
    assert!(!state.lock().expect("tracking state poisoned").closed);
    gate.add_permits(1);
    tokio::time::timeout(Duration::from_secs(2), completed.notified())
        .await
        .expect("worker survives caller Drop");
    let state = state.lock().expect("tracking state poisoned");
    assert!(state.closed);
    assert_eq!(state.close_calls, 1);
    let counters = pool.state();
    assert_eq!(counters.active_count, 0);
    assert_eq!(counters.idle_count, 0);
    assert_eq!(counters.close_count, 1);
    assert_eq!(counters.recycle_count, 0);
    assert_eq!(counters.recycle_error_count, 0);
    assert_eq!(counters.discard_count, 1);
    assert_eq!(counters.destroy_count, 1);
}

/// 复用通知与统计顺序在多线程下保持一致，下一租约不继承前次通知。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task6_close_completion_reuse_publishes_idle_and_counters() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder).await;
    let mut first_id = None;
    for expected in 1..=32 {
        let mut connection = pool.get().await.expect("same holder can be borrowed");
        let id = connection.id();
        assert_eq!(*first_id.get_or_insert(id), id);
        tokio::time::timeout(Duration::from_secs(2), connection.close())
            .await
            .expect("reuse does not wait for a future physical close")
            .expect("reuse succeeds");
        let counters = pool.state();
        assert_eq!(counters.active_count, 0);
        assert_eq!(counters.idle_count, 1);
        assert_eq!(counters.close_count, expected);
        assert_eq!(counters.recycle_count, expected);
        assert_eq!(counters.destroy_count, 0);
    }
    assert_eq!(factory.created_count(), 1);
    assert_eq!(
        factory
            .state(0)
            .lock()
            .expect("tracking state poisoned")
            .close_calls,
        0
    );
}

/// 驱动关闭错误保留 Java 回收的吞错合同，同时只执行一次真实关闭尝试。
#[tokio::test]
async fn task6_close_completion_driver_error_finishes_once() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder.max_use_count(1)).await;
    let mut connection = pool.get().await.expect("connection opens");
    let state = factory.state(0);
    state
        .lock()
        .expect("tracking state poisoned")
        .fail_operation = Some("close");
    tokio::time::timeout(Duration::from_secs(2), connection.close())
        .await
        .expect("failed driver attempt notifies completion")
        .expect("physical close error remains swallowed");
    connection
        .close()
        .await
        .expect("second close does not retry driver");
    drop(connection);
    let state = state.lock().expect("tracking state poisoned");
    assert!(state.closed);
    assert_eq!(state.close_calls, 1);
    let counters = pool.state();
    assert_eq!(counters.active_count, 0);
    assert_eq!(counters.idle_count, 0);
    assert_eq!(counters.close_count, 1);
    assert_eq!(counters.discard_count, 1);
    assert_eq!(counters.destroy_count, 1);
}
