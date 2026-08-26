//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Comprehensive coverage boost tests for druid-core.
//!
//! Targets: config.rs (47 uncovered), filter.rs (20 uncovered),
//! `pooled_connection.rs` (31 uncovered), connection.rs (52 uncovered),
//! `connection_holder.rs` (29 uncovered), error.rs (16 uncovered),
//! value.rs (16 uncovered), `exception_sorter.rs` (11 uncovered).

use druid::core::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ══════════════════════════════════════════════════════════════════
// Mock Connection + ConnectionExt + ConnectionFactory
// ══════════════════════════════════════════════════════════════════

struct MockConnForPool;

druid_rdbc::impl_standard_wrapper!(MockConnForPool => Connection);

#[async_trait::async_trait]
impl Connection for MockConnForPool {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: Some(42),
            row_count: None,
        })
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Ok(vec![Row::new(vec![
            Value::Int(1),
            Value::String("test".into()),
        ])])
    }
    async fn begin(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn commit(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn rollback(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("mock-pool".to_owned())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(true)
    }
    async fn set_auto_commit(&mut self, _auto_commit: bool) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        self.ping().await?;
        Ok(true)
    }
}

// ══════════════════════════════════════════════════════════════════
// 1. config.rs: PoolConfig default + all builder methods (47 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 2. filter.rs: ExtendedFilter default methods (20 lines)
// ══════════════════════════════════════════════════════════════════

struct MockExtendedFilter;

#[async_trait::async_trait]
impl ExtendedFilter for MockExtendedFilter {}

// ══════════════════════════════════════════════════════════════════
// 3. pooled_connection.rs: exec/fetch/begin/commit/rollback/ping (31 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 4. connection_holder.rs: state machine (29 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 5. error.rs: Display for all variants (16 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 6. value.rs: Display + From conversions (16 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 7. exception_sorter.rs (11 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 8. connection.rs: All default method bodies (52 lines)
// ══════════════════════════════════════════════════════════════════

struct MockConnForDefaults;

druid_rdbc::impl_standard_wrapper!(MockConnForDefaults => Connection);

#[async_trait::async_trait]
impl Connection for MockConnForDefaults {
    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        Ok(true)
    }
    async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult::default())
    }
    async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Ok(vec![])
    }
    async fn begin(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn commit(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn rollback(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

// ══════════════════════════════════════════════════════════════════
// 9. connection_factory.rs
// ══════════════════════════════════════════════════════════════════

struct MockConnectionFactory;

#[async_trait::async_trait]
impl ConnectionFactory for MockConnectionFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(MockConnForDefaults))
    }
    async fn validate(&self, _conn: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        Ok(())
    }
}

// ══════════════════════════════════════════════════════════════════
// 10. MetaData struct
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 11. StatementType enum
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 12. ConnState
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 13. ValidConnectionChecker
// ══════════════════════════════════════════════════════════════════

#[test]
fn task6_coverage_boost_ext_test_pool_config_default_values() {
    let cfg = PoolConfig::default();
    assert_eq!(cfg.name, "");
    assert_eq!(cfg.url, "");
    assert_eq!(cfg.driver_name, "");
    assert_eq!(cfg.username, "");
    assert_eq!(cfg.password, "");
    assert!(cfg.connect_properties.is_empty());
    assert_eq!(cfg.max_open, 8);
    assert_eq!(cfg.min_idle, 0);
    assert_eq!(cfg.initial_size, 0);
    assert_eq!(cfg.acquire_timeout, Duration::MAX);
    assert_eq!(cfg.max_lifetime, Duration::MAX);
    assert_eq!(cfg.eviction_interval, Duration::from_mins(1));
    assert_eq!(cfg.min_evictable_idle, Duration::from_mins(30));
    assert!(!cfg.test_on_borrow);
    assert!(!cfg.test_on_return);
    assert!(!cfg.test_while_idle);
    assert!(cfg.validation_query.is_none());
    assert_eq!(cfg.validation_query_timeout, Duration::ZERO);
    assert!(!cfg.keep_alive);
    assert_eq!(cfg.keep_alive_interval, Duration::from_mins(2));
    assert!(!cfg.leak_detection);
    assert_eq!(cfg.leak_threshold, Duration::from_mins(5));
    assert!(!cfg.leak_stack_trace);
    assert!(cfg.default_auto_commit.is_none());
    assert!(cfg.default_read_only.is_none());
    assert!(cfg.default_transaction_isolation.is_none());
    assert!(!cfg.pool_prepared_statements);
    assert_eq!(cfg.max_pool_prepared_statements_per_connection, 10);
    assert_eq!(cfg.slow_sql_threshold, Duration::from_secs(2));
    assert!(cfg.merge_sql);
    assert!(!cfg.connection_stack_trace);
    assert!(cfg.use_unfair_lock);
    assert!(!cfg.break_after_acquire_failure);
    assert_eq!(cfg.connection_error_retry_attempts, 1);
    assert!(!cfg.async_close_connection);
    assert!(cfg.valid_connection_check_class.is_none());
    assert!(cfg.dup_close_log_enable);
}

#[test]
fn task6_coverage_boost_ext_test_pool_config_builder_all_methods() {
    let cfg = PoolConfig::builder()
        .name("test-pool")
        .url("rdbc:postgresql://localhost/test")
        .driver_name("postgres")
        .username("admin")
        .password("secret")
        .max_open(20)
        .min_idle(5)
        .initial_size(3)
        .acquire_timeout(Duration::from_secs(10))
        .max_lifetime(Duration::from_hours(1))
        .eviction_interval(Duration::from_secs(30))
        .min_evictable_idle(Duration::from_mins(10))
        .test_on_borrow(true)
        .test_on_return(true)
        .test_while_idle(true)
        .validation_query("SELECT 1")
        .keep_alive(true)
        .leak_detection(true)
        .leak_threshold(Duration::from_mins(10))
        .slow_sql_threshold(Duration::from_secs(1))
        .pool_prepared_statements(true)
        .default_auto_commit(true)
        .break_after_acquire_failure(true)
        .connection_error_retry_attempts(3)
        .async_close_connection(true)
        .build();

    assert_eq!(cfg.name, "test-pool");
    assert_eq!(cfg.url, "rdbc:postgresql://localhost/test");
    assert_eq!(cfg.driver_name, "postgres");
    assert_eq!(cfg.username, "admin");
    assert_eq!(cfg.password, "secret");
    assert_eq!(cfg.max_open, 20);
    assert_eq!(cfg.min_idle, 5);
    assert_eq!(cfg.initial_size, 3);
    assert_eq!(cfg.acquire_timeout, Duration::from_secs(10));
    assert_eq!(cfg.max_lifetime, Duration::from_hours(1));
    assert_eq!(cfg.eviction_interval, Duration::from_secs(30));
    assert_eq!(cfg.min_evictable_idle, Duration::from_mins(10));
    assert!(cfg.test_on_borrow);
    assert!(cfg.test_on_return);
    assert!(cfg.test_while_idle);
    assert_eq!(cfg.validation_query.as_deref(), Some("SELECT 1"));
    assert!(cfg.keep_alive);
    assert!(cfg.leak_detection);
    assert_eq!(cfg.leak_threshold, Duration::from_mins(10));
    assert_eq!(cfg.slow_sql_threshold, Duration::from_secs(1));
    assert!(cfg.pool_prepared_statements);
    assert_eq!(cfg.default_auto_commit, Some(true));
    assert!(cfg.break_after_acquire_failure);
    assert_eq!(cfg.connection_error_retry_attempts, 3);
    assert!(cfg.async_close_connection);
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_extended_filter_all_statement_property_events() {
    let f = MockExtendedFilter;
    let events = vec![
        StatementPropertyEvent::SetQueryTimeout(30),
        StatementPropertyEvent::GetQueryTimeout,
        StatementPropertyEvent::GetUpdateCount,
        StatementPropertyEvent::SetMaxRows(100),
        StatementPropertyEvent::GetMaxRows,
        StatementPropertyEvent::SetMaxFieldSize(1024),
        StatementPropertyEvent::GetMaxFieldSize,
        StatementPropertyEvent::SetFetchDirection(1),
        StatementPropertyEvent::GetFetchDirection,
        StatementPropertyEvent::SetFetchSize(10),
        StatementPropertyEvent::GetFetchSize,
        StatementPropertyEvent::IsPoolable,
        StatementPropertyEvent::IsClosed,
        StatementPropertyEvent::GetMoreResults,
        StatementPropertyEvent::GetResultSetConcurrency,
        StatementPropertyEvent::GetResultSetType,
        StatementPropertyEvent::GetResultSetHoldability,
        StatementPropertyEvent::GetGeneratedKeys,
        StatementPropertyEvent::ClearWarnings,
        StatementPropertyEvent::SetCursorName("csr".into()),
        StatementPropertyEvent::AddBatch("INSERT INTO t VALUES (1)".into()),
    ];
    for event in &events {
        f.on_statement_property_event(event).await.unwrap();
    }
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_extended_filter_all_clob_events() {
    let f = MockExtendedFilter;
    let events = vec![
        ClobEvent::Length,
        ClobEvent::GetSubString(0, 10),
        ClobEvent::SetString(0, "hello".into()),
        ClobEvent::Truncate(5),
        ClobEvent::Free,
    ];
    for event in &events {
        f.on_clob_event(event).await.unwrap();
    }
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_extended_filter_all_datasource_events() {
    let f = MockExtendedFilter;
    let events = vec![
        DataSourceEvent::GetConnection,
        DataSourceEvent::GetConnectionWithAuth("user".into(), "pass".into()),
        DataSourceEvent::ReleaseConnection,
        DataSourceEvent::Log("test log".into()),
    ];
    for event in &events {
        f.on_datasource_event(event).await.unwrap();
    }
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_extended_filter_config_from_properties() {
    let mut f = MockExtendedFilter;
    let mut props = HashMap::new();
    props.insert("key".into(), "value".into());
    f.config_from_properties(&props).await.unwrap();
}

#[test]
fn task6_coverage_boost_ext_test_extended_filter_is_wrapper_for() {
    let f = MockExtendedFilter;
    assert!(!f.is_wrapper_for("anything"));
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_pooled_connection_all_methods() {
    let holder = ConnectionHolder::with_standard_connection(
        Box::new(MockConnForPool),
        42,
        Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let return_fn_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = Arc::clone(&return_fn_called);
    let mut pc = PooledConnection::with_standard_holder(
        holder,
        "coverage-ext".to_owned(),
        None,
        false,
        false,
        Box::new(move |_, _| {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
            true
        }),
    );
    assert_eq!(pc.id(), 42);
    assert!(!pc.driver_name().unwrap().is_empty());
    let result = pc.exec("INSERT INTO t VALUES (1)", vec![]).await.unwrap();
    assert_eq!(result.rows_affected, 1);
    assert_eq!(result.last_insert_id, Some(42));
    let rows = pc.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
    pc.begin().await.unwrap();
    pc.commit().await.unwrap();
    pc.begin().await.unwrap();
    pc.rollback().await.unwrap();
    pc.ping().await.unwrap();
    assert!(Connection::ping(&mut pc).await.is_ok());
    pc.recycle();
    assert!(return_fn_called.load(std::sync::atomic::Ordering::Relaxed));
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_pooled_connection_close_returns_exactly_once() {
    let returns = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let callback_returns = Arc::clone(&returns);
    let holder =
        ConnectionHolder::with_standard_connection(Box::new(MockConnForPool), 1, Duration::ZERO, 0)
            .await
            .unwrap();
    let mut pc = PooledConnection::with_standard_holder(
        holder,
        "coverage-ext".to_owned(),
        None,
        false,
        false,
        Box::new(move |_, _| {
            callback_returns.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            true
        }),
    );
    pc.close().await.unwrap();
    pc.close().await.unwrap();
    assert!(pc.is_recycled());
    drop(pc);
    assert_eq!(returns.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_pooled_connection_drop_calls_return() {
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag_for_callback = Arc::clone(&flag);
    {
        let holder = ConnectionHolder::with_standard_connection(
            Box::new(MockConnForPool),
            1,
            Duration::ZERO,
            0,
        )
        .await
        .unwrap();
        let _connection = PooledConnection::with_standard_holder(
            holder,
            "coverage-ext".to_owned(),
            None,
            false,
            false,
            Box::new(move |_, _| {
                flag_for_callback.store(true, std::sync::atomic::Ordering::Relaxed);
                true
            }),
        );
    }
    assert!(flag.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn task6_coverage_boost_ext_test_connection_holder_full_state_machine() {
    let h = ConnectionHolder::new(1);
    assert_eq!(h.state(), ConnectionState::Idle);

    // Idle -> Active
    assert!(h.mark_active());
    assert_eq!(h.state(), ConnectionState::Active);
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Active -> Idle
    assert!(h.mark_idle());
    assert_eq!(h.state(), ConnectionState::Idle);

    // Idle -> Active again (should succeed)
    assert!(h.mark_active());
    assert_eq!(h.state(), ConnectionState::Active);
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 2);

    // Active -> Active should fail
    assert!(!h.mark_active());

    // try_transition: success
    assert!(h.try_transition(ConnectionState::Active, ConnectionState::Idle));
    // try_transition: failure (wrong from state - Idle can't become Active if already Idle... but it can)
    // Use a transition that won't match
    h.try_transition(ConnectionState::Idle, ConnectionState::Closing);
    assert!(!h.try_transition(ConnectionState::Idle, ConnectionState::Active)); // already Closing
}

#[test]
fn task6_coverage_boost_ext_test_connection_holder_is_alive_variants() {
    let h = ConnectionHolder::new(1);
    assert!(h.is_alive(Duration::from_mins(1)));
    let d = h.held_duration();
    assert!(d < Duration::from_secs(1));
}

#[test]
fn task6_coverage_boost_ext_test_connection_holder_all_states_display() {
    let h = ConnectionHolder::new(1);
    assert_eq!(h.state(), ConnectionState::Idle);

    h.mark_active();
    assert_eq!(h.state(), ConnectionState::Active);

    h.mark_idle();
    assert_eq!(h.state(), ConnectionState::Idle);

    // Force close
    h.try_transition(ConnectionState::Idle, ConnectionState::Closing);
    assert_eq!(h.state(), ConnectionState::Closing);

    h.try_transition(ConnectionState::Closing, ConnectionState::Closed);
    assert_eq!(h.state(), ConnectionState::Closed);
    assert!(!h.is_alive(Duration::from_mins(1)));

    // Error state
    let h2 = ConnectionHolder::new(2);
    h2.try_transition(ConnectionState::Idle, ConnectionState::Error);
    assert_eq!(h2.state(), ConnectionState::Error);
    assert!(!h2.is_alive(Duration::from_mins(1)));

    // Validating state
    let h3 = ConnectionHolder::new(3);
    h3.try_transition(ConnectionState::Idle, ConnectionState::Validating);
    assert_eq!(h3.state(), ConnectionState::Validating);
}

#[test]
fn task6_coverage_boost_ext_test_connection_holder_fingerprint() {
    let h = ConnectionHolder::new(1);
    assert!(h.last_fingerprint.lock().unwrap().is_none());
    *h.last_fingerprint.lock().unwrap() = Some(12345);
    assert_eq!(*h.last_fingerprint.lock().unwrap(), Some(12345));
}

#[test]
fn task6_coverage_boost_ext_test_error_display_all_variants() {
    let errors = vec![
        RdbcError::DataSourceClosed {
            close_time_millis: 0,
        },
        RdbcError::AcquireTimeout,
        RdbcError::PoolExhausted,
        RdbcError::ValidationFailed("timeout".into()),
        RdbcError::ConnectionLeaked {
            id: 1,
            held_for: Duration::from_mins(1),
        },
        RdbcError::ConnectionDiscarded,
        RdbcError::DriverError("bad driver".into()),
        RdbcError::SqlParseError("syntax error".into()),
        RdbcError::WallViolation("injection detected".into()),
        RdbcError::DataSourceNotFound("ds1".into()),
        RdbcError::Other("something".into()),
    ];
    for e in &errors {
        let s = format!("{e}");
        assert!(!s.is_empty(), "Display for {e:?} should not be empty");
    }
    assert!(format!(
        "{}",
        RdbcError::DataSourceClosed {
            close_time_millis: 0
        }
    )
    .contains("closed"));
    assert!(format!("{}", RdbcError::AcquireTimeout).contains("timed out"));
    assert!(format!("{}", RdbcError::PoolExhausted).contains("exhausted"));
    assert!(format!("{}", RdbcError::ValidationFailed("x".into())).contains('x'));
    assert!(format!(
        "{}",
        RdbcError::ConnectionLeaked {
            id: 5,
            held_for: Duration::from_secs(10)
        }
    )
    .contains('5'));
    assert!(format!("{}", RdbcError::DriverError("d".into())).contains('d'));
    assert!(format!("{}", RdbcError::SqlParseError("p".into())).contains('p'));
    assert!(format!("{}", RdbcError::WallViolation("w".into())).contains('w'));
    assert!(format!("{}", RdbcError::DataSourceNotFound("n".into())).contains('n'));
    assert!(format!("{}", RdbcError::Other("o".into())).contains('o'));
}

#[test]
fn task6_coverage_boost_ext_test_error_from_string_conversions() {
    let e: RdbcError = "test".into();
    assert_eq!(e, RdbcError::Other("test".into()));
    let e2: RdbcError = String::from("hello").into();
    assert_eq!(e2, RdbcError::Other("hello".into()));
}

#[test]
fn task6_coverage_boost_ext_test_error_is_std_error() {
    let e = RdbcError::DataSourceClosed {
        close_time_millis: 0,
    };
    let _: &dyn std::error::Error = &e;
}

#[test]
fn task6_coverage_boost_ext_test_value_display_all_variants() {
    assert_eq!(format!("{}", Value::Null), "NULL");
    assert_eq!(format!("{}", Value::Bool(true)), "true");
    assert_eq!(format!("{}", Value::Bool(false)), "false");
    assert_eq!(format!("{}", Value::Int(42)), "42");
    assert_eq!(format!("{}", Value::Float(3.125)), "3.125");
    assert_eq!(format!("{}", Value::String("hello".into())), "'hello'");
    assert_eq!(format!("{}", Value::Bytes(vec![1, 2, 3])), "<3 bytes>");
}

#[test]
fn task6_coverage_boost_ext_test_value_from_conversions() {
    let v: Value = true.into();
    assert_eq!(v, Value::Bool(true));
    let v: Value = 42i64.into();
    assert_eq!(v, Value::Int(42));
    let v: Value = 42i32.into();
    assert_eq!(v, Value::Int(42));
    let v: Value = 3.125f64.into();
    assert_eq!(v, Value::Float(3.125));
    let v: Value = String::from("test").into();
    assert_eq!(v, Value::String("test".into()));
    let v: Value = "hello".into();
    assert_eq!(v, Value::String("hello".into()));
    let v: Value = vec![1u8, 2, 3].into();
    assert_eq!(v, Value::Bytes(vec![1, 2, 3]));
}

#[test]
fn task6_coverage_boost_ext_test_exception_sorter_mysql_variants() {
    let sorter = MySqlExceptionSorter;
    assert!(!sorter.is_exception_fatal(&SqlException::driver(0, "normal")));
    assert!(sorter.is_exception_fatal(&SqlException::driver(1042, "hostname")));
    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "Communications link failure")));
    assert!(!sorter.is_exception_fatal(&SqlException::driver(999, "Connection refused")));
    assert!(!sorter.is_exception_fatal(&SqlException::driver(1062, "duplicate")));
}

#[test]
fn task6_coverage_boost_ext_test_exception_sorter_pg_variants() {
    let sorter = PgExceptionSorter;
    assert!(sorter.is_exception_fatal(&SqlException::driver(0, "recoverable").recoverable()));
    assert!(sorter.is_exception_fatal(
        &SqlException::driver(0, "connection failure").with_sql_state("08000")
    ));
    assert!(!sorter.is_exception_fatal(&SqlException::driver(0, "no state")));
    assert!(!sorter.is_exception_fatal(&SqlException::driver(0, "syntax").with_sql_state("42601")));
}

#[test]
fn task6_coverage_boost_ext_test_exception_sorter_null() {
    let sorter = NullExceptionSorter;
    assert!(!sorter.is_exception_fatal(&SqlException::driver(0, "anything")));
    assert!(!sorter.is_exception_fatal(&SqlException::driver(57001, "shutdown")));
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_connection_trait_all_defaults() {
    let mut c = MockConnForDefaults;
    let savepoint = Savepoint {
        id: 1,
        name: Some("sp1".to_owned()),
    };
    assert!(c.rollback_to(&savepoint).await.is_err());
    assert!(c.set_savepoint().await.is_err());
    assert!(c.set_savepoint_named("sp").await.is_err());
    assert!(c.release_savepoint(&savepoint).await.is_err());
    c.abort().await.unwrap();
    assert!(c.is_closed().is_err());
    assert!(c.auto_commit().is_err());
    assert!(matches!(
        c.set_auto_commit(false).await,
        Err(RdbcError::UnsupportedOperation { .. })
    ));
    assert!(c.read_only().is_err());
    assert!(matches!(
        c.set_read_only(true).await,
        Err(RdbcError::UnsupportedOperation { .. })
    ));
    assert!(c.transaction_isolation().is_err());
    assert!(matches!(
        c.set_transaction_isolation(druid_rdbc::connection::TransactionIsolation::REPEATABLE_READ,)
            .await,
        Err(RdbcError::UnsupportedOperation { .. })
    ));
    assert!(c.catalog().is_err());
    assert!(matches!(
        c.set_catalog(Some("mydb")).await,
        Err(RdbcError::UnsupportedOperation { .. })
    ));
    assert!(c.schema().is_err());
    assert!(matches!(
        c.set_schema(Some("public")).await,
        Err(RdbcError::UnsupportedOperation { .. })
    ));
    assert!(c.driver_name().is_err());
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_connection_ext_all_defaults() {
    let mut c = MockConnForDefaults;
    let options = druid_rdbc::statement::StatementOptions::default();
    assert!(c.create_statement(options).await.is_err());
    assert!(c.prepare_statement("SELECT 1", options).await.is_err());
    assert!(c.prepare_call("CALL sp()", options).await.is_err());
    assert!(c.database_meta_data().await.is_err());
    assert!(c.database_product_name().await.is_err());
    assert!(c.database_product_version().await.is_err());
    assert!(c.driver_major_version().is_err());
    assert!(c.driver_minor_version().is_err());
    assert!(c.holdability().is_err());
    assert!(c
        .set_holdability(druid_rdbc::statement::ResultSetHoldability::CloseAtCommit)
        .await
        .is_err());
    assert!(c.set_client_info("key", Some("value")).await.is_err());
    assert!(c.client_info("key").is_err());
    assert!(c.warnings().await.is_err());
    assert!(c.clear_warnings().await.is_err());
    assert_eq!(c.native_sql("SELECT 1").await.unwrap(), "SELECT 1");
    assert!(c.set_network_timeout(Duration::from_secs(5)).await.is_err());
    assert!(c.network_timeout().is_err());
    assert!(c.type_map().is_err());
    assert!(c
        .set_type_map(druid_rdbc::types::RdbcTypeMap::new())
        .await
        .is_err());
    assert!(c.auto_commit().is_err());
    assert!(c.read_only().is_err());
    assert!(c.transaction_isolation().is_err());
    assert!(c.catalog().is_err());
    assert!(c.schema().is_err());
    assert!(c.is_closed().is_err());
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_connection_factory_create_and_validate() {
    let factory = MockConnectionFactory;
    let mut conn = factory.create().await.unwrap();
    assert!(conn.ping().await.is_ok());
    factory.validate(&mut conn).await.unwrap();
}

#[test]
fn task6_coverage_boost_ext_test_meta_data_default() {
    let md = MetaData::default();
    assert!(md.database_product_name.is_empty());
    assert!(md.database_product_version.is_empty());
    assert!(md.driver_name.is_empty());
    assert!(md.driver_version.is_empty());
    assert_eq!(md.driver_major_version, 0);
    assert_eq!(md.driver_minor_version, 0);
}
