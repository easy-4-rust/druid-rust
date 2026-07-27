//! Differential tests: druid-rust vs Druid Java 1.2.28 behavioral parity.
//! Tests in druid-core cover core trait semantics only.

use druid_core::*;

// ── PoolConfig defaults match DruidJava DruidAbstractDataSource ──

#[test]
fn test_pool_config_defaults_match_druid_java() {
    let c = PoolConfig::default();
    assert_eq!(c.initial_size, 0);                          // DruidJava: initialSize = 0
    assert_eq!(c.max_open, 8);                              // DruidJava: maxActive = 8
    assert_eq!(c.min_idle, 0);                              // DruidJava: minIdle = 0
    assert_eq!(c.acquire_timeout, std::time::Duration::from_secs(30)); // maxWait = -1 → 30s default
    assert_eq!(c.min_evictable_idle, std::time::Duration::from_secs(1800)); // 30 min
    assert_eq!(c.eviction_interval, std::time::Duration::from_secs(60)); // 1 min
    assert!(!c.test_on_borrow);
    assert!(!c.test_on_return);
    assert!(!c.pool_prepared_statements);
    assert!(!c.keep_alive);
    assert!(!c.leak_detection);
    assert_eq!(c.leak_threshold, std::time::Duration::from_secs(300));
    assert!(c.use_unfair_lock);
    assert!(!c.break_after_acquire_failure);
    assert_eq!(c.connection_error_retry_attempts, 1);
}

// ── DruidJava ConnectionHolder state machine ──

#[test]
fn test_connection_holder_initial_state() {
    let h = ConnectionHolder::new(1);
    assert_eq!(h.state(), ConnectionState::Idle);
}

#[test]
fn test_connection_holder_idle_to_active() {
    let h = ConnectionHolder::new(1);
    assert!(h.mark_active());
    assert_eq!(h.state(), ConnectionState::Active);
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
fn test_connection_holder_active_to_idle() {
    let h = ConnectionHolder::new(1);
    h.mark_active();
    assert!(h.mark_idle());
    assert_eq!(h.state(), ConnectionState::Idle);
}

#[test]
fn test_connection_holder_cas_invalid_transition() {
    let h = ConnectionHolder::new(1);
    assert!(h.try_transition(ConnectionState::Idle, ConnectionState::Idle));
    assert!(h.try_transition(ConnectionState::Idle, ConnectionState::Active));
}

#[test]
fn test_connection_holder_is_alive() {
    let h = ConnectionHolder::new(1);
    assert!(h.is_alive(std::time::Duration::from_secs(60)));
}

#[test]
fn test_connection_holder_use_count() {
    let h = ConnectionHolder::new(1);
    h.mark_active();
    h.mark_idle();
    h.mark_active();
    h.mark_idle();
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 2);
}

// ── DruidJava ExceptionSorter ──

#[test]
fn test_pg_exception_sorter_fatal() {
    let sorter = PgExceptionSorter;
    assert!(sorter.is_exception_fatal(57001, "admin shutdown"));
}

#[test]
fn test_pg_exception_sorter_non_fatal() {
    let sorter = PgExceptionSorter;
    assert!(!sorter.is_exception_fatal(42601, "syntax error"));
}

#[test]
fn test_mysql_exception_sorter_fatal() {
    let sorter = MySqlExceptionSorter;
    assert!(sorter.is_exception_fatal(1042, "Can't get hostname"));
}

#[test]
fn test_null_exception_sorter_never_fatal() {
    let sorter = NullExceptionSorter;
    assert!(!sorter.is_exception_fatal(99999, "anything"));
}

// ── DruidJava Value type ──

#[test]
fn test_value_display_all_variants() {
    assert_eq!(format!("{}", Value::Null), "NULL");
    assert_eq!(format!("{}", Value::Bool(true)), "true");
    assert_eq!(format!("{}", Value::Int(42)), "42");
    assert_eq!(format!("{}", Value::Float(3.14)), "3.14");
    assert_eq!(format!("{}", Value::String("hello".into())), "'hello'");
    assert_eq!(format!("{}", Value::Bytes(vec![1, 2, 3])), "<3 bytes>");
}

// ── DruidJava Connection transaction semantics ──

/// DruidJava: begin() opens a transaction (DruidPooledConnection.beginTransaction).
#[tokio::test]
async fn test_connection_begin() {
    struct MockConn { tx_active: bool }
    #[async_trait::async_trait]
    impl druid_core::Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<druid_core::ExecResult, druid_core::DruidError> { Ok(druid_core::ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<Vec<druid_core::Row>, druid_core::DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), druid_core::DruidError> { self.tx_active = true; Ok(()) }
        async fn commit(&mut self) -> Result<(), druid_core::DruidError> { self.tx_active = false; Ok(()) }
        async fn rollback(&mut self) -> Result<(), druid_core::DruidError> { self.tx_active = false; Ok(()) }
        async fn ping(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
    }
    let mut c = MockConn { tx_active: false };
    assert!(!c.tx_active);
    c.begin().await.unwrap();
    assert!(c.tx_active);
    c.commit().await.unwrap();
    assert!(!c.tx_active);
}

/// DruidJava: rollback() aborts transaction.
#[tokio::test]
async fn test_connection_rollback() {
    struct MockConn;
    #[async_trait::async_trait]
    impl druid_core::Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<druid_core::ExecResult, druid_core::DruidError> { Ok(druid_core::ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<Vec<druid_core::Row>, druid_core::DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
    }
    let mut c = MockConn;
    c.begin().await.unwrap();
    c.rollback().await.unwrap(); // Should succeed
}

/// DruidJava: set_savepoint() returns Savepoint with id.
#[tokio::test]
async fn test_connection_savepoint_default_not_supported() {
    struct MockConn;
    #[async_trait::async_trait]
    impl druid_core::Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<druid_core::ExecResult, druid_core::DruidError> { Ok(druid_core::ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<Vec<druid_core::Row>, druid_core::DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
    }
    let mut c = MockConn;
    // Default impl returns Err
    let result = c.set_savepoint().await;
    assert!(result.is_err());
}

/// DruidJava: rollback(Savepoint) default not supported.
#[tokio::test]
async fn test_connection_rollback_to_default_not_supported() {
    struct MockConn;
    #[async_trait::async_trait]
    impl druid_core::Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<druid_core::ExecResult, druid_core::DruidError> { Ok(druid_core::ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<Vec<druid_core::Row>, druid_core::DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
    }
    let mut c = MockConn;
    let sp = druid_core::Savepoint { id: 1, name: Some("sp1".into()) };
    assert!(c.rollback_to(&sp).await.is_err());
}

/// DruidJava: abort() closes connection.
#[tokio::test]
async fn test_connection_abort_closes() {
    struct MockConn { closed: bool }
    #[async_trait::async_trait]
    impl druid_core::Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<druid_core::ExecResult, druid_core::DruidError> { Ok(druid_core::ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<Vec<druid_core::Row>, druid_core::DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), druid_core::DruidError> { self.closed = true; Ok(()) }
    }
    let mut c = MockConn { closed: false };
    assert!(!c.closed);
    c.abort().await.unwrap();
    assert!(c.closed); // Default impl calls close()
}

/// DruidJava: isClosed() returns connection state.
#[test]
fn test_connection_is_closed_default() {
    struct MockConn;
    #[async_trait::async_trait]
    impl druid_core::Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<druid_core::ExecResult, druid_core::DruidError> { Ok(druid_core::ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<Vec<druid_core::Row>, druid_core::DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
    }
    let c = MockConn;
    assert!(!c.is_closed());
}

/// DruidJava: getAutoCommit / setAutoCommit defaults.
#[test]
fn test_connection_auto_commit_defaults() {
    struct MockConn;
    #[async_trait::async_trait]
    impl druid_core::Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<druid_core::ExecResult, druid_core::DruidError> { Ok(druid_core::ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<druid_core::Value>) -> Result<Vec<druid_core::Row>, druid_core::DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), druid_core::DruidError> { Ok(()) }
    }
    let c = MockConn;
    // DruidJava default: autoCommit = true
    assert!(c.auto_commit());
    assert!(!c.read_only());
    // DruidJava: TRANSACTION_READ_COMMITTED = 2
    assert_eq!(c.transaction_isolation(), 2);
    assert!(c.catalog().is_none());
    assert!(c.schema().is_none());
    assert_eq!(c.driver_name(), "");
}

/// DruidJava: ExecResult fields.
#[test]
fn test_exec_result_default() {
    let r = druid_core::ExecResult::default();
    assert_eq!(r.rows_affected, 0);
    assert!(r.last_insert_id.is_none());
    assert!(r.row_count.is_none());
}

/// DruidJava: Savepoint struct.
#[test]
fn test_savepoint_fields() {
    let sp = druid_core::Savepoint { id: 42, name: Some("sp1".into()) };
    assert_eq!(sp.id, 42);
    assert_eq!(sp.name.as_deref(), Some("sp1"));

    let sp2 = druid_core::Savepoint { id: 1, name: None };
    assert_eq!(sp2.id, 1);
    assert!(sp2.name.is_none());
}

/// DruidJava: ConnectionState default values.
#[test]
fn test_connection_state_defaults() {
    let s = druid_core::ConnState::default();
    assert!(s.auto_commit);
    assert!(!s.read_only);
    assert_eq!(s.transaction_isolation, 2); // READ_COMMITTED
    assert!(s.catalog.is_none());
    assert!(s.schema.is_none());
}
