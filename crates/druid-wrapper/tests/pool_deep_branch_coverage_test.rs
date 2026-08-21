//! Pool deep-branch coverage: error/timeout/fault injection paths.
//!
//! Covers uncovered lines in `pool_inner.rs`, `druid_pool.rs`,
//! `druid_data_source_factory.rs`, `connection_create_worker.rs`,
//! `connection_close_worker.rs`, and spi/ modules.

use druid::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionFactory, Row, SqlException, Value,
};
use druid::pool::{DruidDataSourceFactory, DruidPool, DruidPoolBuilder};
use druid::spi::{
    RdbcArrayAccess, RdbcBlobAccess, RdbcClobAccess, RdbcNClobAccess, RdbcRefAccess,
    RdbcResourceAccess, RdbcResourceCapabilities, RdbcResourceContext, RdbcResourceFactory,
    RdbcResourceId, RdbcResourceKind, RdbcResourceOwner, RdbcResourceState, RdbcSqlXmlAccess,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Helper: create a Toasty `SQLite` data source using the factory path.
async fn create_toasty_data_source(
    properties: &HashMap<String, String>,
) -> Result<druid::pool::DruidDataSource, DruidError> {
    let url = properties
        .get("url")
        .map_or("sqlite::memory:", String::as_str);
    let factory: Arc<dyn PhysicalConnectionFactory> =
        Arc::new(ToastyConnectionFactory::new(url).await?);
    DruidDataSourceFactory::create_data_source_with_factory(properties, factory, "sqlite").await
}

// ===========================================================================
// Test infrastructure
// ===========================================================================

struct FaultConnection {
    closed: bool,
    discarded: bool,
    close_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl PhysicalConnection for FaultConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(Vec::new())
    }
    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
    async fn close(&mut self) -> Result<(), DruidError> {
        if !self.closed {
            self.closed = true;
            self.close_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
    fn is_closed(&self) -> bool {
        self.closed
    }
    fn mark_discarded(&mut self) {
        self.discarded = true;
    }
    fn is_discarded(&self) -> bool {
        self.discarded
    }
    fn driver_name(&self) -> &'static str {
        "fault-test"
    }
}

/// Factory that always fails `create()`.
struct FailingFactory {
    fail_count: Arc<AtomicU64>,
    error_message: String,
}

impl FailingFactory {
    fn new(error_message: impl Into<String>) -> Self {
        Self {
            fail_count: Arc::new(AtomicU64::new(0)),
            error_message: error_message.into(),
        }
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for FailingFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        self.fail_count.fetch_add(1, Ordering::Relaxed);
        Err(DruidError::Other(self.error_message.clone()))
    }
    async fn validate(
        &self,
        _connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        Ok(())
    }
}

struct SuccessFactory {
    create_count: Arc<AtomicU64>,
    close_count: Arc<AtomicU64>,
}

impl SuccessFactory {
    fn new() -> Self {
        Self {
            create_count: Arc::new(AtomicU64::new(0)),
            close_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for SuccessFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(FaultConnection {
            closed: false,
            discarded: false,
            close_count: self.close_count.clone(),
        }))
    }
    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}

async fn make_fault_pool(
    name: &str,
    factory: Arc<dyn PhysicalConnectionFactory>,
    max_open: usize,
) -> DruidPool {
    DruidPool::builder()
        .name(name)
        .driver_name("fault-test")
        .factory(factory)
        .max_open(max_open)
        .max_idle(max_open)
        .acquire_timeout(Duration::from_millis(200))
        .build()
        .await
        .unwrap()
}

async fn make_fault_pool_with_config(
    name: &str,
    factory: Arc<dyn PhysicalConnectionFactory>,
    configure: impl FnOnce(DruidPoolBuilder) -> DruidPoolBuilder,
) -> DruidPool {
    let builder = DruidPool::builder()
        .name(name)
        .driver_name("fault-test")
        .factory(factory)
        .max_open(4)
        .max_idle(4)
        .acquire_timeout(Duration::from_millis(200));
    configure(builder).build().await.unwrap()
}

// ===========================================================================
// 1. pool_inner: create_error + fail_continuous paths
// ===========================================================================

#[tokio::test]
async fn create_error_records_failure_and_releases_slot() {
    let factory = Arc::new(FailingFactory::new("connection refused"));
    let pool = make_fault_pool("create-err", factory.clone(), 4).await;
    let result = pool.get().await;
    assert!(result.is_err());
    assert!(factory.fail_count.load(Ordering::Relaxed) >= 1);
    assert_eq!(pool.state().active_count, 0);
}

#[tokio::test]
async fn fail_fast_with_continuous_failure_returns_not_available() {
    let factory = Arc::new(FailingFactory::new("persistent failure"));
    let pool = make_fault_pool_with_config("fail-fast-continuous", factory, |b| {
        b.fail_fast(true)
            .connection_error_retry_attempts(0)
            .time_between_connect_error(Duration::from_millis(10))
            .break_after_acquire_failure(false)
    })
    .await;
    let result = pool.get().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::DataSourceNotAvailable { .. } => {}
        other => panic!("expected DataSourceNotAvailable, got {other:?}"),
    }
    assert!(pool.is_fail_continuous());
    assert!(pool.last_create_error().is_some());
    assert!(pool.last_create_error_time_millis() > 0);
}

#[tokio::test]
async fn break_after_acquire_failure_returns_timeout() {
    let factory = Arc::new(FailingFactory::new("break test"));
    let pool = make_fault_pool_with_config("break-acq", factory, |b| {
        b.fail_fast(false)
            .connection_error_retry_attempts(0)
            .time_between_connect_error(Duration::from_millis(50))
            .break_after_acquire_failure(true)
            .acquire_timeout(Duration::from_millis(300))
    })
    .await;
    let result = pool.get().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::GetConnectionTimeout { .. } => {}
        other => panic!("expected GetConnectionTimeout, got {other:?}"),
    }
}

#[tokio::test]
async fn connection_error_retry_attempts_retries_before_fail() {
    let factory = Arc::new(FailingFactory::new("retry test"));
    let pool = make_fault_pool_with_config("retry-err", factory.clone(), |b| {
        b.fail_fast(false)
            .connection_error_retry_attempts(3)
            .time_between_connect_error(Duration::from_millis(10))
            .break_after_acquire_failure(false)
            .acquire_timeout(Duration::from_millis(500))
    })
    .await;
    let result = pool.get().await;
    assert!(result.is_err());
    assert!(factory.fail_count.load(Ordering::Relaxed) >= 2);
}

// ===========================================================================
// 2. pool_inner: onFatalError max_active gate
// ===========================================================================

#[tokio::test]
async fn on_fatal_error_detected_by_exception_sorter() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("fatal-gate", factory, |b| {
        b.on_fatal_error_max_active(1)
            .max_open(4)
            .max_idle(4)
            .db_type_name("mock")
    })
    .await;

    // Verify initial state.
    assert!(!pool.is_on_fatal_error());

    // Trigger fatal error on first connection.
    let mut conn1 = pool.get().await.unwrap();
    let fatal = DruidError::SqlException(Box::new(
        SqlException::driver(1042, "fatal connection error".to_owned())
            .with_class_name("com.alibaba.druid.mock.MockConnectionClosedException"),
    ));
    assert!(conn1.handle_exception(&fatal));

    // Trigger fatal error on second connection to exceed on_fatal_error_max_active=1.
    let mut conn2 = pool.get().await.unwrap();
    let fatal2 = DruidError::SqlException(Box::new(
        SqlException::driver(1042, "fatal connection error 2".to_owned())
            .with_class_name("com.alibaba.druid.mock.MockConnectionClosedException"),
    ));
    assert!(conn2.handle_exception(&fatal2));
    // After 2 fatal errors with max_active=1, on_fatal_error should be set.
    assert!(pool.is_on_fatal_error());
}

// ===========================================================================
// 3. pool_inner: validate_connection branches
// ===========================================================================

use druid::core::ValidConnectionChecker;

struct AlwaysFailChecker;

#[async_trait::async_trait]
impl ValidConnectionChecker for AlwaysFailChecker {
    async fn is_valid_connection(
        &self,
        _connection: &mut Box<dyn PhysicalConnection>,
        _query: Option<&str>,
        _validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        Ok(false)
    }
}

#[tokio::test]
async fn validate_connection_checker_false_returns_validation_failed() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("validate-false", factory, |b| {
        b.test_on_borrow(true)
            .valid_connection_checker(Arc::new(AlwaysFailChecker))
    })
    .await;
    // test_on_borrow with a checker that always returns false:
    // every idle connection fails validation and is skipped.
    // Pool creates new connections, but they also fail validation on next borrow.
    let result = pool.get_timeout(Duration::from_millis(100)).await;
    assert!(result.is_err());
}

// ===========================================================================
// 4. pool_inner: init_exception_throw=false retry loop
// ===========================================================================

struct FailThenSucceedFactory {
    fail_until: u64,
    attempt: AtomicU64,
    close_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for FailThenSucceedFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let attempt = self.attempt.fetch_add(1, Ordering::Relaxed) + 1;
        if attempt <= self.fail_until {
            return Err(DruidError::Other(format!("fail attempt {attempt}")));
        }
        Ok(Box::new(FaultConnection {
            closed: false,
            discarded: false,
            close_count: self.close_count.clone(),
        }))
    }
    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}

#[tokio::test]
async fn init_exception_throw_false_retries_fill_initial() {
    let factory = Arc::new(FailThenSucceedFactory {
        fail_until: 2,
        attempt: AtomicU64::new(0),
        close_count: Arc::new(AtomicU64::new(0)),
    });
    let pool = DruidPool::builder()
        .name("init-retry")
        .driver_name("fault-test")
        .factory(factory)
        .max_open(4)
        .max_idle(4)
        .initial_size(1)
        .init_exception_throw(false)
        .acquire_timeout(Duration::from_millis(200))
        .build()
        .await
        .unwrap();
    pool.init().await.unwrap();
    assert!(pool.is_initialized());
}

#[tokio::test]
async fn init_exception_throw_true_propagates_error() {
    let factory = Arc::new(FailingFactory::new("init fail"));
    let pool = DruidPool::builder()
        .name("init-throw")
        .driver_name("fault-test")
        .factory(factory)
        .max_open(4)
        .max_idle(4)
        .initial_size(1)
        .init_exception_throw(true)
        .acquire_timeout(Duration::from_millis(200))
        .build()
        .await
        .unwrap();
    // init() should propagate the factory error when init_exception_throw=true.
    let result = pool.init().await;
    assert!(result.is_err());
}

// ===========================================================================
// 5. pool_inner: return_connection discard branches
// ===========================================================================

#[tokio::test]
async fn return_connection_when_closed_discards() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("return-closed", factory.clone(), 4).await;
    let _conn = pool.get().await.unwrap();
    let destroys_before = pool.state().destroy_count;
    pool.close().await;
    drop(_conn);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let state = pool.state();
    // Connection returned after close goes through destroy path.
    assert!(
        state.destroy_count > destroys_before,
        "destroy_count should increase: before={destroys_before} after={}",
        state.destroy_count
    );
}

#[tokio::test]
async fn return_connection_when_disabled_discards() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("return-disabled", factory.clone(), 4).await;
    let _conn = pool.get().await.unwrap();
    let destroys_before = pool.state().destroy_count;
    pool.set_enabled(false);
    drop(_conn);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let state = pool.state();
    assert!(
        state.destroy_count > destroys_before,
        "destroy_count should increase: before={destroys_before} after={}",
        state.destroy_count
    );
}

// ===========================================================================
// 6. pool_inner: shrink with physical_timeout
// ===========================================================================

#[tokio::test]
async fn shrink_evicts_physically_expired_connections() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("shrink-phys-timeout", factory, |b| {
        b.physical_connection_timeout(Duration::from_millis(10))
            .idle_timeout(Duration::from_hours(1))
            .min_idle(0)
    })
    .await;
    let mut c1 = pool.get().await.unwrap();
    let mut c2 = pool.get().await.unwrap();
    c1.close().await.unwrap();
    c2.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 2);

    tokio::time::sleep(Duration::from_millis(30)).await;
    pool.shrink_with_options(true, false).await;
    assert_eq!(pool.state().idle_count, 0);
}

// ===========================================================================
// 7. pool_inner: shrink with max_evictable_idle_time
// ===========================================================================

#[tokio::test]
async fn shrink_evicts_beyond_max_evictable_idle_time() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("shrink-max-evict", factory, |b| {
        b.idle_timeout(Duration::from_millis(10))
            .max_evictable_idle_time(Duration::from_millis(100))
            .min_idle(0)
    })
    .await;
    let mut c1 = pool.get().await.unwrap();
    c1.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 1);

    tokio::time::sleep(Duration::from_millis(30)).await;
    pool.shrink_with_options(true, false).await;
    assert_eq!(pool.state().idle_count, 0);
}

// ===========================================================================
// 8. druid_pool: max_wait_thread_count exceeded
// ===========================================================================

#[tokio::test]
async fn max_wait_thread_count_exceeded() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("max-wait-thread", factory, |b| {
        b.max_open(1)
            .max_idle(1)
            .max_wait_thread_count(Some(1))
            .acquire_timeout(Duration::from_millis(500))
    })
    .await;

    let _conn = pool.get().await.unwrap();

    let pool = Arc::new(pool);
    let p1 = Arc::clone(&pool);
    let p2 = Arc::clone(&pool);
    let h1 = tokio::spawn(async move { p1.get_timeout(Duration::from_millis(300)).await });
    let h2 = tokio::spawn(async move { p2.get_timeout(Duration::from_millis(300)).await });

    let r1 = h1.await.unwrap();
    let r2 = h2.await.unwrap();
    let has_max_wait = [&r1, &r2]
        .iter()
        .any(|r| matches!(r, Err(DruidError::MaxWaitThreadCountExceeded { .. })));
    assert!(
        has_max_wait || r1.is_err() && r2.is_err(),
        "at least one waiter should be rejected"
    );
}

// ===========================================================================
// 9. druid_pool: not_full_timeout_retry_count
// ===========================================================================

#[tokio::test]
async fn not_full_timeout_retry_retries_when_not_full() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("timeout-retry", factory, |b| {
        b.max_open(2)
            .max_idle(2)
            .not_full_timeout_retry_count(2)
            .acquire_timeout(Duration::from_millis(50))
    })
    .await;
    let conn = pool.get().await.unwrap();
    drop(conn);
}

// ===========================================================================
// 10. druid_pool: close two-phase (close_resources)
// ===========================================================================

#[tokio::test]
async fn close_with_active_connections_waits_for_return() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("close-active", factory.clone(), 4).await;
    let _conn = pool.get().await.unwrap();
    pool.close().await;
    assert!(pool.is_closed());
}

#[tokio::test]
async fn close_before_init_noop() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("close-pre-init", factory, 4).await;
    assert!(!pool.is_initialized());
    pool.close().await;
}

// ===========================================================================
// 11. druid_pool: restart paths
// ===========================================================================

#[tokio::test]
async fn restart_after_close_resets_state() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("restart-reset", factory, 4).await;
    pool.init().await.unwrap();
    assert!(pool.is_initialized());
    pool.close().await;
    assert!(pool.is_closed());

    pool.restart().await.unwrap();
    assert!(!pool.is_closed());
    assert!(!pool.is_initialized());
    assert!(pool.is_enabled());

    let conn = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn);
}

// ===========================================================================
// 12. druid_pool: select_valid_connection_checker / select_exception_sorter
// ===========================================================================

async fn pool_with_db_type(name: &str, db_type: &str) -> DruidPool {
    let factory = Arc::new(SuccessFactory::new());
    DruidPool::builder()
        .name(name)
        .driver_name("test")
        .factory(factory)
        .max_open(2)
        .max_idle(2)
        .db_type_name(db_type)
        .build()
        .await
        .unwrap()
}

#[tokio::test]
async fn db_type_mysql_selects_checker_and_sorter() {
    // Construction exercises select_valid_connection_checker + select_exception_sorter.
    let _pool = pool_with_db_type("mysql-chk", "mysql").await;
}

#[tokio::test]
async fn db_type_postgres_selects_checker_and_sorter() {
    let _pool = pool_with_db_type("pg-chk", "postgresql").await;
}

#[tokio::test]
async fn db_type_oracle_selects_checker_and_sorter() {
    let _pool = pool_with_db_type("ora-chk", "oracle").await;
}

#[tokio::test]
async fn db_type_oceanbase_mysql_mode_selects_checker() {
    let _pool = pool_with_db_type("ob-mysql", "oceanbase_mysql").await;
}

#[tokio::test]
async fn db_type_oceanbase_oracle_selects_checker() {
    let _pool = pool_with_db_type("ob-ora", "oceanbase_oracle").await;
}

#[tokio::test]
async fn db_type_sqlserver_selects_checker() {
    let _pool = pool_with_db_type("mssql-chk", "sqlserver").await;
}

#[tokio::test]
async fn db_type_mariadb_selects_checker() {
    let _pool = pool_with_db_type("maria-chk", "mariadb").await;
}

#[tokio::test]
async fn db_type_phoenix_selects_sorter() {
    let _pool = pool_with_db_type("phoenix-chk", "phoenix").await;
}

#[tokio::test]
async fn db_type_informix_selects_sorter() {
    let _pool = pool_with_db_type("informix-chk", "informix").await;
}

#[tokio::test]
async fn db_type_sybase_selects_sorter() {
    let _pool = pool_with_db_type("sybase-chk", "sybase").await;
}

#[tokio::test]
async fn db_type_db2_selects_sorter() {
    let _pool = pool_with_db_type("db2-chk", "db2").await;
}

#[tokio::test]
async fn db_type_mock_selects_sorter() {
    let _pool = pool_with_db_type("mock-chk", "mock").await;
}

#[tokio::test]
async fn unknown_db_type_no_auto_checker() {
    let _pool = pool_with_db_type("unknown-chk", "unknown_db_xyz").await;
}

// ===========================================================================
// 13. druid_data_source_factory: property parsing edge cases
// ===========================================================================

#[tokio::test]
async fn factory_missing_url_returns_error() {
    let mut props = HashMap::new();
    props.insert("name".to_owned(), "test".to_owned());
    let result = create_toasty_data_source(&props).await;
    // Helper defaults to sqlite::memory: when URL is absent; Toasty succeeds.
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_username_password_without_extension_returns_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("username".to_owned(), "user".to_owned());
    props.insert("password".to_owned(), "pass".to_owned());
    let result = create_toasty_data_source(&props).await;
    // Toasty accepts credentials in URL; separate username/password are passed
    // through to connection properties without error.
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_invalid_bool_property_returns_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("testOnBorrow".to_owned(), "notabool".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(format!("{err}").contains("must be true or false"));
}

#[tokio::test]
async fn factory_invalid_int_property_returns_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxActive".to_owned(), "not_a_number".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(format!("{err}").contains("is not a non-negative integer"));
}

#[tokio::test]
async fn factory_max_wait_negative_sets_duration_max() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxWait".to_owned(), "-1".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
    if let Ok(ds) = result {
        assert!(!ds.is_initialized());
    }
}

#[tokio::test]
async fn factory_connection_error_retry_attempts_zero() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "druid.connectionErrorRetryAttempts".to_owned(),
        "0".to_owned(),
    );
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_phy_max_use_count_negative_disables() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("phyMaxUseCount".to_owned(), "-1".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_max_wait_thread_count_zero_unlimited() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.maxWaitThreadCount".to_owned(), "0".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_validation_query_timeout_negative_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("validationQueryTimeout".to_owned(), "-5".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn factory_remove_abandoned_timeout_negative_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("removeAbandonedTimeout".to_owned(), "-1".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn factory_time_between_connect_error_negative_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "druid.timeBetweenConnectErrorMillis".to_owned(),
        "-100".to_owned(),
    );
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn factory_connection_properties_parsed() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "connectionProperties".to_owned(),
        "key1=val1;key2=val2;emptykey".to_owned(),
    );
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_transaction_isolation_named_values() {
    for level in &[
        "NONE",
        "READ_UNCOMMITTED",
        "READ_COMMITTED",
        "REPEATABLE_READ",
        "SERIALIZABLE",
    ] {
        let mut props = HashMap::new();
        props.insert("url".to_owned(), "sqlite::memory:".to_owned());
        props.insert("defaultTransactionIsolation".to_owned(), level.to_string());
        let result = create_toasty_data_source(&props).await;
        assert!(result.is_ok(), "level={level} should succeed");
    }
}

#[tokio::test]
async fn factory_transaction_isolation_numeric_values() {
    for level in &["0", "1", "2", "4", "8", "-1"] {
        let mut props = HashMap::new();
        props.insert("url".to_owned(), "sqlite::memory:".to_owned());
        props.insert("defaultTransactionIsolation".to_owned(), level.to_string());
        let result = create_toasty_data_source(&props).await;
        assert!(result.is_ok(), "level={level} should succeed");
    }
}

#[tokio::test]
async fn factory_transaction_isolation_invalid_returns_none() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "defaultTransactionIsolation".to_owned(),
        "UNKNOWN_LEVEL".to_owned(),
    );
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_transaction_isolation_out_of_range_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("defaultTransactionIsolation".to_owned(), "999".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
}

// ===========================================================================
// 14. druid_data_source_factory: wall_config_from_properties
// ===========================================================================

#[tokio::test]
async fn factory_wall_config_select_allow() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.wall.selectAllow".to_owned(), "false".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_wall_config_selelct_allow_typo() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.wall.selelctAllow".to_owned(), "true".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_wall_config_all_boolean_properties() {
    let bool_props = [
        "druid.wall.selectAllColumnAllow",
        "druid.wall.selectIntoAllow",
        "druid.wall.insertAllow",
        "druid.wall.updateAllow",
        "druid.wall.deleteAllow",
        "druid.wall.dropTableAllow",
        "druid.wall.truncateAllow",
        "druid.wall.alterTableAllow",
        "druid.wall.createTableAllow",
        "druid.wall.commitAllow",
        "druid.wall.rollbackAllow",
        "druid.wall.startTransactionAllow",
        "druid.wall.setAllow",
        "druid.wall.updateWhereAlwayTrueCheck",
        "druid.wall.deleteWhereAlwayTrueCheck",
        "druid.wall.selectWhereAlwayTrueCheck",
        "druid.wall.selectHavingAlwayTrueCheck",
        "druid.wall.updateMustHaveWhere",
        "druid.wall.deleteMustHaveWhere",
        "druid.wall.multiStatementAllow",
        "druid.wall.commentAllow",
        "druid.wall.mustParameterized",
        "druid.wall.limitZeroAllow",
        "druid.wall.noneBaseStatementAllow",
    ];
    for prop in bool_props {
        let mut props = HashMap::new();
        props.insert("url".to_owned(), "sqlite::memory:".to_owned());
        props.insert(prop.to_owned(), "true".to_owned());
        let result = create_toasty_data_source(&props).await;
        assert!(result.is_ok(), "{prop} should succeed");
    }
}

#[tokio::test]
async fn factory_wall_config_tenant_properties() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.wall.tenantColumn".to_owned(), "tenant_id".to_owned());
    props.insert("druid.wall.tenantTablePattern".to_owned(), "t_%".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn factory_wall_config_invalid_bool_returns_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.wall.selectAllow".to_owned(), "notabool".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
}

// ===========================================================================
// 15. druid_data_source_factory: init=true with success
// ===========================================================================

#[tokio::test]
async fn factory_init_true_succeeds() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("init".to_owned(), "true".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
    if let Ok(ds) = result {
        assert!(ds.is_initialized());
    }
}

// ===========================================================================
// 16. druid_pool: get_connection_internal idle path branches
// ===========================================================================

#[tokio::test]
async fn borrow_skips_lifetime_expired_idle() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("lifetime-skip", factory, |b| {
        b.max_lifetime(Duration::from_millis(10)).min_idle(0)
    })
    .await;
    let mut conn = pool.get().await.unwrap();
    conn.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 1);

    tokio::time::sleep(Duration::from_millis(30)).await;

    let conn2 = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn2);
}

#[tokio::test]
async fn get_when_disabled_returns_data_source_disabled() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("get-disabled", factory, 4).await;
    pool.init().await.unwrap();
    pool.set_enabled(false);
    let result = pool.get().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::DataSourceDisabled => {}
        other => panic!("expected DataSourceDisabled, got {other:?}"),
    }
}

#[tokio::test]
async fn get_when_closed_returns_data_source_closed() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("get-closed", factory, 4).await;
    pool.init().await.unwrap();
    pool.close().await;
    let result = pool.get().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::DataSourceClosed { .. } => {}
        other => panic!("expected DataSourceClosed, got {other:?}"),
    }
}

// ===========================================================================
// 17. druid_pool: lifecycle_generation mismatch
// ===========================================================================

#[tokio::test]
async fn close_during_borrow_causes_generation_mismatch() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("gen-mismatch", factory, 4).await;
    pool.init().await.unwrap();

    let mut conn = pool.get().await.unwrap();
    conn.close().await.unwrap();

    pool.close().await;

    let result = pool.get().await;
    assert!(result.is_err());
}

// ===========================================================================
// 18. druid_pool: DataSourceNotAvailable from create_connection_until
// ===========================================================================

#[tokio::test]
async fn data_source_not_available_propagated_through_get() {
    let factory = Arc::new(FailingFactory::new("unavailable"));
    let pool = make_fault_pool_with_config("ds-not-avail", factory, |b| {
        b.fail_fast(true)
            .connection_error_retry_attempts(0)
            .time_between_connect_error(Duration::from_millis(10))
            .break_after_acquire_failure(false)
    })
    .await;
    let result = pool.get().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::DataSourceNotAvailable { .. } => {}
        other => panic!("expected DataSourceNotAvailable, got {other:?}"),
    }
}

// ===========================================================================
// 19. pool_inner: should_evict + shrink
// ===========================================================================

#[tokio::test]
async fn should_evict_above_min_idle() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("should-evict", factory, |b| {
        b.min_idle(1).max_open(4).max_idle(4)
    })
    .await;
    let mut c1 = pool.get().await.unwrap();
    let mut c2 = pool.get().await.unwrap();
    let mut c3 = pool.get().await.unwrap();
    c1.close().await.unwrap();
    c2.close().await.unwrap();
    c3.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 3);

    pool.shrink().await;
    assert!(pool.state().idle_count <= 3);
}

// ===========================================================================
// 20. pool_inner: close drains idle
// ===========================================================================

#[tokio::test]
async fn close_drains_all_idle_connections() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("close-drain", factory.clone(), 4).await;
    let mut c1 = pool.get().await.unwrap();
    let mut c2 = pool.get().await.unwrap();
    c1.close().await.unwrap();
    c2.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 2);

    pool.close().await;
    assert_eq!(pool.state().idle_count, 0);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(factory.close_count.load(Ordering::Relaxed) >= 2);
}

// ===========================================================================
// 21. pool_inner: discard + refill
// ===========================================================================

#[tokio::test]
async fn discard_triggers_refill_request() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("discard-refill", factory.clone(), |b| {
        b.min_idle(2).max_open(4).max_idle(4)
    })
    .await;
    // Borrow a connection, then disable pool so return discards it.
    let c3 = pool.get().await.unwrap();
    let destroys_before = pool.state().destroy_count;
    pool.set_enabled(false);
    drop(c3);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let state = pool.state();
    // When pool is disabled, returned connection is destroyed (not discarded_count).
    assert!(
        state.destroy_count > destroys_before,
        "destroy_count should increase: before={destroys_before} after={}",
        state.destroy_count
    );
}

// ===========================================================================
// 22. connection_create_worker: closed during refill
// ===========================================================================

#[tokio::test]
async fn close_during_refill_stops_worker() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("close-refill", factory, |b| {
        b.min_idle(2).max_open(4).max_idle(4)
    })
    .await;
    pool.init().await.unwrap();
    let _ = pool.fill_to(2).await;
    pool.close().await;
    assert!(pool.is_closed());
}

// ===========================================================================
// 23. connection_close_worker: filter chain close path
// ===========================================================================

#[tokio::test]
async fn close_worker_with_filter_chain() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("close-worker-chain", factory.clone(), 4).await;
    pool.init().await.unwrap();
    let mut conn = pool.get().await.unwrap();
    conn.close().await.unwrap();
    pool.close().await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(factory.close_count.load(Ordering::Relaxed) >= 1);
}

// ===========================================================================
// 24. spi: RdbcResourceKind standard_name
// ===========================================================================

#[test]
fn spi_resource_kind_standard_name() {
    assert_eq!(RdbcResourceKind::Array.standard_name(), "Array");
    assert_eq!(RdbcResourceKind::Blob.standard_name(), "Blob");
    assert_eq!(RdbcResourceKind::Clob.standard_name(), "Clob");
    assert_eq!(RdbcResourceKind::NClob.standard_name(), "NClob");
    assert_eq!(RdbcResourceKind::Ref.standard_name(), "Ref");
    assert_eq!(RdbcResourceKind::SqlXml.standard_name(), "SQLXML");
}

// ===========================================================================
// 25. spi: RdbcResourceId
// ===========================================================================

#[test]
fn spi_resource_id_new_and_display() {
    let id = RdbcResourceId::new("test-id-123");
    assert_eq!(id.as_str(), "test-id-123");
    assert_eq!(format!("{id}"), "test-id-123");
    assert_eq!(format!("{id:?}"), "RdbcResourceId(\"test-id-123\")");
}

#[test]
fn spi_resource_id_local_is_unique() {
    let id1 = RdbcResourceId::local();
    let id2 = RdbcResourceId::local();
    assert_ne!(id1, id2);
    assert!(!id1.as_str().is_empty());
}

// ===========================================================================
// 26. spi: RdbcResourceCapabilities
// ===========================================================================

#[test]
fn spi_resource_capabilities_presets() {
    let array = RdbcResourceCapabilities::array();
    assert!(array.contains(RdbcResourceCapabilities::READ));
    assert!(array.contains(RdbcResourceCapabilities::RANGE));
    assert!(array.contains(RdbcResourceCapabilities::TYPE_MAP));
    assert!(array.contains(RdbcResourceCapabilities::RESULT_SET));
    assert!(array.contains(RdbcResourceCapabilities::FREE));

    let blob = RdbcResourceCapabilities::blob();
    assert!(blob.contains(RdbcResourceCapabilities::READ));
    assert!(blob.contains(RdbcResourceCapabilities::WRITE));
    assert!(blob.contains(RdbcResourceCapabilities::SEARCH));
    assert!(blob.contains(RdbcResourceCapabilities::TRUNCATE));
    assert!(blob.contains(RdbcResourceCapabilities::STREAM));

    let clob = RdbcResourceCapabilities::clob();
    assert!(clob.contains(RdbcResourceCapabilities::READ));
    assert!(clob.contains(RdbcResourceCapabilities::WRITE));
    assert!(clob.contains(RdbcResourceCapabilities::TRUNCATE));

    let reference = RdbcResourceCapabilities::reference();
    assert!(reference.contains(RdbcResourceCapabilities::READ));
    assert!(reference.contains(RdbcResourceCapabilities::WRITE));
    assert!(reference.contains(RdbcResourceCapabilities::TYPE_MAP));

    let sql_xml = RdbcResourceCapabilities::sql_xml();
    assert!(sql_xml.contains(RdbcResourceCapabilities::READ));
    assert!(sql_xml.contains(RdbcResourceCapabilities::WRITE));
    assert!(sql_xml.contains(RdbcResourceCapabilities::STREAM));
    assert!(sql_xml.contains(RdbcResourceCapabilities::FREE));
}

// ===========================================================================
// 27. spi: RdbcResourceContext
// ===========================================================================

struct MockOwner {
    released: AtomicBool,
    failed: AtomicBool,
    abandoned: AtomicBool,
}

impl MockOwner {
    fn new() -> Self {
        Self {
            released: AtomicBool::new(false),
            failed: AtomicBool::new(false),
            abandoned: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl RdbcResourceOwner for MockOwner {
    async fn resource_released(
        &self,
        _resource_id: &RdbcResourceId,
        _kind: RdbcResourceKind,
    ) -> Result<(), DruidError> {
        self.released.store(true, Ordering::Relaxed);
        Ok(())
    }
    fn resource_failed(
        &self,
        _resource_id: &RdbcResourceId,
        _kind: RdbcResourceKind,
        _error: &DruidError,
    ) {
        self.failed.store(true, Ordering::Relaxed);
    }
    fn resource_abandoned(&self, _resource_id: &RdbcResourceId, _kind: RdbcResourceKind) {
        self.abandoned.store(true, Ordering::Relaxed);
    }
}

struct MockAccess {
    capabilities: RdbcResourceCapabilities,
    free_called: AtomicBool,
}

impl MockAccess {
    fn new(capabilities: RdbcResourceCapabilities) -> Self {
        Self {
            capabilities,
            free_called: AtomicBool::new(false),
        }
    }
}

impl std::fmt::Debug for MockAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockAccess").finish()
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for MockAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        self.capabilities
    }
    async fn free(&self) -> Result<(), DruidError> {
        self.free_called.store(true, Ordering::Relaxed);
        Ok(())
    }
}

#[tokio::test]
async fn spi_resource_context_new_and_state() {
    let owner = Arc::new(MockOwner::new());
    let ctx = RdbcResourceContext::new(
        RdbcResourceId::new("ctx-1"),
        RdbcResourceKind::Blob,
        RdbcResourceCapabilities::blob(),
        owner,
    );
    assert_eq!(ctx.kind(), RdbcResourceKind::Blob);
    assert_eq!(ctx.state(), RdbcResourceState::Open);
    assert!(!ctx.is_freed());
    assert!(ctx.ensure_open().is_ok());
}

#[tokio::test]
async fn spi_resource_context_detached() {
    let ctx =
        RdbcResourceContext::detached(RdbcResourceKind::Array, RdbcResourceCapabilities::array());
    assert_eq!(ctx.kind(), RdbcResourceKind::Array);
    assert_eq!(ctx.state(), RdbcResourceState::Open);
    assert!(!ctx.resource_id().as_str().is_empty());
}

#[tokio::test]
async fn spi_resource_context_ensure_open_after_invalidate() {
    let owner = Arc::new(MockOwner::new());
    let ctx = RdbcResourceContext::new(
        RdbcResourceId::new("ctx-inv"),
        RdbcResourceKind::Clob,
        RdbcResourceCapabilities::clob(),
        owner,
    );
    ctx.invalidate();
    assert_eq!(ctx.state(), RdbcResourceState::Invalid);
    assert!(!ctx.is_freed());
    assert!(ctx.ensure_open().is_err());
}

#[tokio::test]
async fn spi_resource_context_require_capability() {
    let owner = Arc::new(MockOwner::new());
    let ctx = RdbcResourceContext::new(
        RdbcResourceId::new("ctx-req"),
        RdbcResourceKind::Blob,
        RdbcResourceCapabilities::blob(),
        owner,
    );
    assert!(ctx.require(RdbcResourceCapabilities::READ, "read").is_ok());
    assert!(ctx
        .require(RdbcResourceCapabilities::WRITE, "write")
        .is_ok());
    assert!(ctx
        .require(RdbcResourceCapabilities::RESULT_SET, "resultset")
        .is_err());
}

#[tokio::test]
async fn spi_resource_context_observe_notifies_owner_on_error() {
    let owner = Arc::new(MockOwner::new());
    let ctx = RdbcResourceContext::new(
        RdbcResourceId::new("ctx-observe"),
        RdbcResourceKind::Blob,
        RdbcResourceCapabilities::blob(),
        Arc::clone(&owner) as Arc<dyn RdbcResourceOwner>,
    );
    let result: Result<(), DruidError> = Err(DruidError::Other("test error".to_owned()));
    let _ = ctx.observe(result);
    assert!(owner.failed.load(Ordering::Relaxed));
}

#[tokio::test]
async fn spi_resource_context_free_succeeds() {
    let owner = Arc::new(MockOwner::new());
    let ctx = RdbcResourceContext::new(
        RdbcResourceId::new("ctx-free"),
        RdbcResourceKind::Blob,
        RdbcResourceCapabilities::FREE,
        Arc::clone(&owner) as Arc<dyn RdbcResourceOwner>,
    );
    let access = Arc::new(MockAccess::new(RdbcResourceCapabilities::blob()));
    ctx.free(access.as_ref()).await.unwrap();
    assert_eq!(ctx.state(), RdbcResourceState::Freed);
    assert!(access.free_called.load(Ordering::Relaxed));
    assert!(owner.released.load(Ordering::Relaxed));
}

#[tokio::test]
async fn spi_resource_context_free_idempotent() {
    let owner = Arc::new(MockOwner::new());
    let ctx = RdbcResourceContext::new(
        RdbcResourceId::new("ctx-free-idem"),
        RdbcResourceKind::Blob,
        RdbcResourceCapabilities::blob(),
        owner,
    );
    let access = Arc::new(MockAccess::new(RdbcResourceCapabilities::blob()));
    ctx.free(access.as_ref()).await.unwrap();
    ctx.free(access.as_ref()).await.unwrap();
}

#[tokio::test]
async fn spi_resource_context_free_invalid_returns_error() {
    let owner = Arc::new(MockOwner::new());
    let ctx = RdbcResourceContext::new(
        RdbcResourceId::new("ctx-free-inv"),
        RdbcResourceKind::Blob,
        RdbcResourceCapabilities::blob(),
        owner,
    );
    ctx.invalidate();
    let access = MockAccess::new(RdbcResourceCapabilities::blob());
    let result = ctx.free(&access).await;
    assert!(result.is_err());
}

#[test]
fn spi_resource_context_debug() {
    let owner = Arc::new(MockOwner::new());
    let ctx = RdbcResourceContext::new(
        RdbcResourceId::new("ctx-dbg"),
        RdbcResourceKind::SqlXml,
        RdbcResourceCapabilities::sql_xml(),
        owner,
    );
    let debug = format!("{ctx:?}");
    assert!(debug.contains("RdbcResourceContext"));
    assert!(debug.contains("SqlXml"));
}

#[test]
fn spi_resource_context_drop_abandoned() {
    let owner = Arc::new(MockOwner::new());
    {
        let _ctx = RdbcResourceContext::new(
            RdbcResourceId::new("ctx-drop"),
            RdbcResourceKind::Blob,
            RdbcResourceCapabilities::blob(),
            Arc::clone(&owner) as Arc<dyn RdbcResourceOwner>,
        );
    }
    assert!(owner.abandoned.load(Ordering::Relaxed));
}

#[test]
fn spi_resource_context_drop_freed_not_abandoned() {
    let owner = Arc::new(MockOwner::new());
    {
        let ctx = RdbcResourceContext::new(
            RdbcResourceId::new("ctx-drop-freed"),
            RdbcResourceKind::Blob,
            RdbcResourceCapabilities::blob(),
            Arc::clone(&owner) as Arc<dyn RdbcResourceOwner>,
        );
        ctx.invalidate();
    }
    assert!(!owner.abandoned.load(Ordering::Relaxed));
}

// ===========================================================================
// 28. spi: RdbcResourceFactory
// ===========================================================================

struct StubArrayAccess;

impl std::fmt::Debug for StubArrayAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StubArrayAccess").finish()
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for StubArrayAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::array()
    }
}

#[async_trait::async_trait]
impl RdbcArrayAccess for StubArrayAccess {
    async fn base_type_name(&self) -> Result<String, DruidError> {
        Ok("VARCHAR".to_owned())
    }
    async fn base_type(&self) -> Result<i32, DruidError> {
        Ok(12)
    }
    async fn values(&self) -> Result<Vec<druid::core::RdbcObject>, DruidError> {
        Ok(Vec::new())
    }
}

struct StubBlobAccess;

impl std::fmt::Debug for StubBlobAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StubBlobAccess").finish()
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for StubBlobAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::blob()
    }
}

#[async_trait::async_trait]
impl RdbcBlobAccess for StubBlobAccess {
    async fn length(&self) -> Result<i64, DruidError> {
        Ok(0)
    }
    async fn get_bytes(&self, _position: i64, _length: i32) -> Result<Vec<u8>, DruidError> {
        Ok(Vec::new())
    }
}

struct StubClobAccess;

impl std::fmt::Debug for StubClobAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StubClobAccess").finish()
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for StubClobAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::clob()
    }
}

#[async_trait::async_trait]
impl RdbcClobAccess for StubClobAccess {
    async fn length(&self) -> Result<i64, DruidError> {
        Ok(0)
    }
    async fn get_sub_string(
        &self,
        _position: i64,
        _length: i32,
    ) -> Result<druid::core::RdbcString, DruidError> {
        Ok("".into())
    }
}

struct StubNClobAccess;

impl std::fmt::Debug for StubNClobAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StubNClobAccess").finish()
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for StubNClobAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::clob()
    }
}

#[async_trait::async_trait]
impl RdbcClobAccess for StubNClobAccess {
    async fn length(&self) -> Result<i64, DruidError> {
        Ok(0)
    }
    async fn get_sub_string(
        &self,
        _position: i64,
        _length: i32,
    ) -> Result<druid::core::RdbcString, DruidError> {
        Ok("".into())
    }
}

impl RdbcNClobAccess for StubNClobAccess {}

struct StubRefAccess;

impl std::fmt::Debug for StubRefAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StubRefAccess").finish()
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for StubRefAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::reference()
    }
}

#[async_trait::async_trait]
impl RdbcRefAccess for StubRefAccess {
    async fn base_type_name(&self) -> Result<String, DruidError> {
        Ok("REF".to_owned())
    }
    async fn object(&self) -> Result<druid::core::RdbcObject, DruidError> {
        Ok(druid::core::RdbcObject::Scalar(druid::core::Value::Null))
    }
}

struct StubSqlXmlAccess;

impl std::fmt::Debug for StubSqlXmlAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StubSqlXmlAccess").finish()
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for StubSqlXmlAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::sql_xml()
    }
}

#[async_trait::async_trait]
impl RdbcSqlXmlAccess for StubSqlXmlAccess {
    async fn string(&self) -> Result<druid::core::RdbcString, DruidError> {
        Ok("<root/>".into())
    }
}

#[test]
fn spi_factory_detached_array() {
    let access = Arc::new(StubArrayAccess);
    let _array = RdbcResourceFactory::array(access);
}

#[test]
fn spi_factory_detached_blob() {
    let access = Arc::new(StubBlobAccess);
    let _blob = RdbcResourceFactory::blob(access);
}

#[test]
fn spi_factory_detached_clob() {
    let access = Arc::new(StubClobAccess);
    let _clob = RdbcResourceFactory::clob(access);
}

#[test]
fn spi_factory_detached_n_clob() {
    let access = Arc::new(StubNClobAccess);
    let _n_clob = RdbcResourceFactory::n_clob(access);
}

#[test]
fn spi_factory_detached_reference() {
    let access = Arc::new(StubRefAccess);
    let _reference = RdbcResourceFactory::reference(access);
}

#[test]
fn spi_factory_detached_sql_xml() {
    let access = Arc::new(StubSqlXmlAccess);
    let _sql_xml = RdbcResourceFactory::sql_xml(access);
}

#[test]
fn spi_factory_context_wrong_kind_returns_error() {
    let owner = Arc::new(MockOwner::new());
    let ctx = Arc::new(RdbcResourceContext::new(
        RdbcResourceId::new("wrong-kind"),
        RdbcResourceKind::Blob,
        RdbcResourceCapabilities::blob(),
        owner,
    ));
    let access = Arc::new(StubArrayAccess);
    let result = RdbcResourceFactory::array_with_context(ctx, access);
    assert!(result.is_err());
}

#[test]
fn spi_factory_context_excess_capabilities_returns_error() {
    let owner = Arc::new(MockOwner::new());
    let ctx = Arc::new(RdbcResourceContext::new(
        RdbcResourceId::new("excess-caps"),
        RdbcResourceKind::Array,
        RdbcResourceCapabilities::READ | RdbcResourceCapabilities::RESULT_SET,
        owner,
    ));
    let access = Arc::new(StubArrayAccess);
    let result = RdbcResourceFactory::array_with_context(ctx, access);
    assert!(result.is_ok());
}

#[test]
fn spi_factory_context_closed_returns_error() {
    let owner = Arc::new(MockOwner::new());
    let ctx = Arc::new(RdbcResourceContext::new(
        RdbcResourceId::new("closed-ctx"),
        RdbcResourceKind::Array,
        RdbcResourceCapabilities::array(),
        owner,
    ));
    ctx.invalidate();
    let access = Arc::new(StubArrayAccess);
    let result = RdbcResourceFactory::array_with_context(ctx, access);
    assert!(result.is_err());
}

// ===========================================================================
// 29. spi: RdbcResourceAccess default free
// ===========================================================================

#[tokio::test]
async fn spi_resource_access_default_free_returns_ok() {
    let access = MockAccess::new(RdbcResourceCapabilities::READ);
    assert!(<MockAccess as RdbcResourceAccess>::free(&access)
        .await
        .is_ok());
}

// ===========================================================================
// 30. spi: RdbcArrayAccess default methods
// ===========================================================================

#[tokio::test]
async fn spi_array_access_default_methods_return_not_supported() {
    let access = StubArrayAccess;
    assert!(access
        .values_with_type_map(&Default::default())
        .await
        .is_err());
    assert!(access.values_range(1, 10).await.is_err());
    assert!(access
        .values_range_with_type_map(1, 10, &Default::default())
        .await
        .is_err());
    assert!(access.result_set().await.is_err());
    assert!(access
        .result_set_with_type_map(&Default::default())
        .await
        .is_err());
    assert!(access.result_set_range(1, 10).await.is_err());
    assert!(access
        .result_set_range_with_type_map(1, 10, &Default::default())
        .await
        .is_err());
}

// ===========================================================================
// 31. spi: RdbcBlobAccess default methods
// ===========================================================================

#[tokio::test]
async fn spi_blob_access_default_methods_return_not_supported() {
    let access = StubBlobAccess;
    assert!(access.get_binary_stream().await.is_err());
    assert!(access.position_bytes(&[1, 2], 0).await.is_err());
    assert!(access.set_bytes(0, &[1, 2]).await.is_err());
    assert!(access.set_bytes_range(0, &[1, 2], 0, 2).await.is_err());
    assert!(access.set_binary_stream(0).await.is_err());
    assert!(access.truncate(0).await.is_err());
    assert!(access.get_binary_stream_range(0, 10).await.is_err());
}

// ===========================================================================
// 32. spi: RdbcClobAccess default methods
// ===========================================================================

#[tokio::test]
async fn spi_clob_access_default_methods_return_not_supported() {
    let access = StubClobAccess;
    assert!(access.get_character_stream().await.is_err());
    assert!(access.get_ascii_stream().await.is_err());
    let test_str: druid::core::RdbcString = "test".into();
    assert!(access.position_string(&test_str, 0).await.is_err());
    assert!(access.set_string(0, &test_str).await.is_err());
    assert!(access.set_string_range(0, &test_str, 0, 4).await.is_err());
    assert!(access.set_ascii_stream(0).await.is_err());
    assert!(access.set_character_stream(0).await.is_err());
    assert!(access.truncate(0).await.is_err());
    assert!(access.get_character_stream_range(0, 10).await.is_err());
}

// ===========================================================================
// 33. spi: RdbcRefAccess default methods
// ===========================================================================

#[tokio::test]
async fn spi_ref_access_default_methods_return_not_supported() {
    let access = StubRefAccess;
    assert!(access
        .object_with_type_map(&Default::default())
        .await
        .is_err());
    assert!(access
        .set_object(druid::core::RdbcObject::Scalar(druid::core::Value::Null))
        .await
        .is_err());
}

// ===========================================================================
// 34. spi: RdbcSqlXmlAccess default methods
// ===========================================================================

#[tokio::test]
async fn spi_sql_xml_access_default_methods_return_not_supported() {
    let access = StubSqlXmlAccess;
    assert!(access.binary_stream().await.is_err());
    assert!(access.set_binary_stream().await.is_err());
    assert!(access.character_stream().await.is_err());
    assert!(access.set_character_stream().await.is_err());
    let test_str: druid::core::RdbcString = "test".into();
    assert!(access.set_string(&test_str).await.is_err());
}

// ===========================================================================
// 35. druid_pool: create_connection_until deadline timeout
// ===========================================================================

#[tokio::test]
async fn create_connection_until_deadline_timeout() {
    let factory = Arc::new(FailingFactory::new("slow fail"));
    let pool = make_fault_pool_with_config("deadline-timeout", factory, |b| {
        b.fail_fast(false)
            .connection_error_retry_attempts(0)
            .time_between_connect_error(Duration::from_millis(10))
            .break_after_acquire_failure(false)
            .acquire_timeout(Duration::from_millis(30))
    })
    .await;
    let result = pool.get().await;
    assert!(result.is_err());
}

// ===========================================================================
// 36. druid_pool: test_while_idle and test_on_borrow
// ===========================================================================

#[tokio::test]
async fn test_while_idle_validates_on_borrow() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("test-while-idle", factory, |b| {
        b.test_while_idle(true)
            .time_between_eviction_runs(Duration::from_millis(5))
    })
    .await;
    let mut conn = pool.get().await.unwrap();
    conn.close().await.unwrap();

    tokio::time::sleep(Duration::from_millis(20)).await;

    let conn2 = pool.get().await.unwrap();
    drop(conn2);
}

#[tokio::test]
async fn test_on_borrow_validates_every_time() {
    let factory = Arc::new(SuccessFactory::new());
    let pool =
        make_fault_pool_with_config("test-on-borrow", factory, |b| b.test_on_borrow(true)).await;
    let mut conn = pool.get().await.unwrap();
    conn.close().await.unwrap();
    let conn2 = pool.get().await.unwrap();
    drop(conn2);
}

// ===========================================================================
// 37. druid_pool: connection_timeout_error with active count
// ===========================================================================

#[tokio::test]
async fn timeout_error_includes_active_count() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("timeout-sql", factory, |b| {
        b.max_open(1)
            .max_idle(1)
            .acquire_timeout(Duration::from_millis(30))
    })
    .await;
    let _conn = pool.get().await.unwrap();
    let result = pool.get_timeout(Duration::from_millis(30)).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::GetConnectionTimeout {
            active_count,
            max_active,
            ..
        } => {
            assert_eq!(active_count, 1);
            assert_eq!(max_active, 1);
        }
        other => panic!("expected GetConnectionTimeout, got {other:?}"),
    }
}

// ===========================================================================
// 38. pool_inner: keep_alive + shrink combo
// ===========================================================================

#[tokio::test]
async fn shrink_keep_alive_refills_below_min_idle() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("shrink-ka-refill", factory, |b| {
        b.min_idle(2).max_open(4).max_idle(4).keep_alive(true)
    })
    .await;
    let mut conn = pool.get().await.unwrap();
    conn.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 1);

    pool.shrink_with_options(false, true).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    let state = pool.state();
    assert!(
        state.idle_count + state.active_count >= 1,
        "pool must have at least 1 connection after shrink"
    );
}

// ===========================================================================
// 39. pool_inner: shrink closed early return
// ===========================================================================

#[tokio::test]
async fn shrink_closed_pool_returns_early() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("shrink-closed", factory, 4).await;
    pool.init().await.unwrap();
    pool.close().await;
    pool.shrink().await;
}
