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

use druid::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalConnectionFactory, Row, ValidConnectionChecker, Value,
};
use druid::pool::DruidPool;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug)]
struct ConnectionState {
    auto_commit: bool,
    read_only: bool,
    holdability: i32,
    transaction_isolation: u8,
    catalog: Option<String>,
    closed: bool,
    discarded: bool,
    fail_operation: Option<&'static str>,
    events: Vec<&'static str>,
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
            discarded: false,
            fail_operation: None,
            events: Vec::new(),
        }
    }
}

struct TrackingConnection {
    state: Arc<Mutex<ConnectionState>>,
}

impl TrackingConnection {
    fn operation(&self, operation: &'static str) -> Result<(), DruidError> {
        let mut state = self.state.lock().expect("tracking state poisoned");
        state.events.push(operation);
        if state.fail_operation == Some(operation) {
            Err(DruidError::DriverError(format!("{operation} failed")))
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for TrackingConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![Row::new(vec![Value::Int(1)])])
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        self.operation("begin")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        self.operation("commit")
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        // JDBC rollback 不改变 autoCommit，随后 holder.reset 再恢复默认值。
        self.operation("rollback")
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        self.operation("ping")
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        let mut state = self.state.lock().expect("tracking state poisoned");
        if !state.closed {
            state.events.push("close");
            state.closed = true;
        }
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.state.lock().expect("tracking state poisoned").closed
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        PhysicalConnectionCapabilities {
            transactions: true,
            savepoints: false,
            auto_commit: true,
            read_only: true,
            transaction_isolation: true,
            holdability: true,
            clear_warnings: true,
            catalog: false,
            schema: false,
        }
    }

    fn auto_commit(&self) -> bool {
        self.state
            .lock()
            .expect("tracking state poisoned")
            .auto_commit
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        self.operation("set_auto_commit")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .auto_commit = auto_commit;
        Ok(())
    }

    fn read_only(&self) -> bool {
        self.state
            .lock()
            .expect("tracking state poisoned")
            .read_only
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), DruidError> {
        self.operation("set_read_only")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .read_only = read_only;
        Ok(())
    }

    fn transaction_isolation(&self) -> u8 {
        self.state
            .lock()
            .expect("tracking state poisoned")
            .transaction_isolation
    }

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), DruidError> {
        self.operation("set_transaction_isolation")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .transaction_isolation = level;
        Ok(())
    }

    fn holdability(&self) -> i32 {
        self.state
            .lock()
            .expect("tracking state poisoned")
            .holdability
    }

    async fn set_holdability(&mut self, holdability: i32) -> Result<(), DruidError> {
        self.operation("set_holdability")?;
        self.state
            .lock()
            .expect("tracking state poisoned")
            .holdability = holdability;
        Ok(())
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.operation("clear_warnings")
    }

    async fn set_catalog(&mut self, catalog: &str) -> Result<(), DruidError> {
        self.operation("set_catalog")?;
        self.state.lock().expect("tracking state poisoned").catalog = Some(catalog.to_string());
        Ok(())
    }

    fn mark_discarded(&mut self) {
        self.state
            .lock()
            .expect("tracking state poisoned")
            .discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.state
            .lock()
            .expect("tracking state poisoned")
            .discarded
    }

    fn driver_name(&self) -> &str {
        "tracking"
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
        connection: &mut Box<dyn PhysicalConnection>,
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
impl PhysicalConnectionFactory for TrackingFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
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

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        self.validation_count.fetch_add(1, Ordering::Relaxed);
        if self.validation_succeeds.load(Ordering::Relaxed) {
            connection.ping().await
        } else {
            Err(DruidError::ValidationFailed(
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
            .max_lifetime(Duration::from_secs(60))
            .acquire_timeout(Duration::from_secs(1)),
    )
    .build()
    .await
    .expect("tracking pool must build")
}

#[tokio::test]
async fn explicit_close_rolls_back_and_resets_in_java_order() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| builder).await;
    let mut connection = pool.get().await.expect("connection must open");
    let connection_id = connection.id();
    connection.begin().await.expect("transaction must begin");
    connection
        .set_holdability(1)
        .await
        .expect("holdability must change");
    connection
        .set_transaction_isolation(8)
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
    assert!(connection.auto_commit());
    assert!(!connection.read_only());
    assert_eq!(connection.holdability(), 0);
    assert_eq!(connection.transaction_isolation(), 2);
}

#[tokio::test]
async fn read_only_transaction_skips_rollback_then_restores_defaults() {
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
async fn rollback_failure_is_swallowed_counted_and_connection_discarded() {
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
async fn test_on_return_validates_and_invalid_connection_is_not_recycled() {
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
async fn physical_connection_is_discarded_at_configured_max_use_count() {
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
async fn dirty_drop_discards_instead_of_leaking_transaction_to_idle_queue() {
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
async fn configured_isolation_preservation_skips_reset() {
    let factory = Arc::new(TrackingFactory::new());
    let pool = build_pool(factory.clone(), |builder| {
        builder.keep_connection_underlying_transaction_isolation(true)
    })
    .await;
    let mut connection = pool.get().await.expect("connection must open");
    let first_id = connection.id();
    connection
        .set_transaction_isolation(8)
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
    assert_eq!(connection.transaction_isolation(), 8);
}

#[tokio::test]
async fn physical_connection_defaults_are_initialized_in_java_order() {
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
    assert!(connection.auto_commit());
    assert!(connection.read_only());
    assert_eq!(connection.transaction_isolation(), 8);
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
async fn canonical_holder_moves_through_idle_queue_without_losing_java_metadata() {
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
async fn mysql_family_db_type_enables_java_init_schema_restore_policy() {
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
async fn odps_skips_default_auto_commit_initialization() {
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
    assert!(!connection.auto_commit());
    assert!(!factory
        .state(0)
        .lock()
        .expect("tracking state poisoned")
        .events
        .contains(&"set_auto_commit"));
}

#[tokio::test]
async fn initialization_failure_closes_connection_and_releases_capacity() {
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
    assert!(matches!(error, DruidError::DriverError(_)));
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
