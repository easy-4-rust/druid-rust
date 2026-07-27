//! Differential tests: druid-rust vs Druid Java 1.2.28 behavioral parity.
use druid_core::*;

// ── PoolConfig defaults ──
#[test]
fn test_pool_config_defaults_match_druid_java() {
    let c = PoolConfig::default();
    assert_eq!(c.initial_size, 0);
    assert_eq!(c.max_open, 8);
    assert_eq!(c.min_idle, 0);
    assert_eq!(c.acquire_timeout, std::time::Duration::from_secs(30));
    assert_eq!(c.min_evictable_idle, std::time::Duration::from_secs(1800));
    assert_eq!(c.eviction_interval, std::time::Duration::from_secs(60));
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

// ── ConnectionHolder state machine ──
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
fn test_connection_holder_cas() {
    let h = ConnectionHolder::new(1);
    assert!(h.try_transition(ConnectionState::Idle, ConnectionState::Idle));
    assert!(h.try_transition(ConnectionState::Idle, ConnectionState::Active));
}
#[test]
fn test_connection_holder_is_alive() {
    assert!(ConnectionHolder::new(1).is_alive(std::time::Duration::from_secs(60)));
}
#[test]
fn test_connection_holder_use_count() {
    let h = ConnectionHolder::new(1);
    h.mark_active(); h.mark_idle(); h.mark_active(); h.mark_idle();
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 2);
}

// ── ExceptionSorter ──
#[test]
fn test_pg_sorter() { assert!(PgExceptionSorter.is_exception_fatal(57001, "admin shutdown")); }
#[test]
fn test_pg_sorter_non_fatal() { assert!(!PgExceptionSorter.is_exception_fatal(42601, "syntax error")); }
#[test]
fn test_mysql_sorter() { assert!(MySqlExceptionSorter.is_exception_fatal(1042, "Can't get hostname")); }
#[test]
fn test_null_sorter() { assert!(!NullExceptionSorter.is_exception_fatal(99999, "anything")); }

// ── Value display ──
#[test]
fn test_value_display_all() {
    assert_eq!(format!("{}", Value::Null), "NULL");
    assert_eq!(format!("{}", Value::Bool(true)), "true");
    assert_eq!(format!("{}", Value::Int(42)), "42");
    assert_eq!(format!("{}", Value::Float(3.14)), "3.14");
    assert_eq!(format!("{}", Value::String("hello".into())), "'hello'");
    assert_eq!(format!("{}", Value::Bytes(vec![1,2,3])), "<3 bytes>");
}

// ── Connection transaction semantics ──
#[tokio::test]
async fn test_begin_commit() {
    struct M { tx: bool }
    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> { Ok(ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), DruidError> { self.tx = true; Ok(()) }
        async fn commit(&mut self) -> Result<(), DruidError> { self.tx = false; Ok(()) }
        async fn rollback(&mut self) -> Result<(), DruidError> { self.tx = false; Ok(()) }
        async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), DruidError> { Ok(()) }
    }
    let mut m = M { tx: false };
    m.begin().await.unwrap(); assert!(m.tx);
    m.commit().await.unwrap(); assert!(!m.tx);
}

#[tokio::test]
async fn test_rollback() {
    struct M; #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> { Ok(ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), DruidError> { Ok(()) }
    }
    let mut m = M; m.begin().await.unwrap(); m.rollback().await.unwrap();
}

#[tokio::test]
async fn test_savepoint_not_supported() {
    struct M; #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> { Ok(ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), DruidError> { Ok(()) }
    }
    let mut m = M;
    assert!(m.set_savepoint().await.is_err());
    assert!(m.set_savepoint_named("sp1").await.is_err());
    assert!(m.release_savepoint(&Savepoint { id: 1, name: None }).await.is_err());
    assert!(m.rollback_to(&Savepoint { id: 1, name: None }).await.is_err());
}

#[tokio::test]
async fn test_abort_closes() {
    struct M { closed: bool } #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> { Ok(ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), DruidError> { self.closed = true; Ok(()) }
    }
    let mut m = M { closed: false };
    m.abort().await.unwrap();
    assert!(m.closed);
}

#[test]
fn test_connection_defaults() {
    struct M; #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> { Ok(ExecResult::default()) }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> { Ok(vec![]) }
        async fn begin(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn commit(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn rollback(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
        async fn close(&mut self) -> Result<(), DruidError> { Ok(()) }
    }
    let m = M;
    assert!(m.auto_commit());
    assert!(!m.read_only());
    assert_eq!(m.transaction_isolation(), 2);
    assert!(m.catalog().is_none());
    assert!(m.schema().is_none());
    assert!(!m.is_closed());
    assert_eq!(m.driver_name(), "");
}

#[test]
fn test_exec_result_default() {
    let r = ExecResult::default();
    assert_eq!(r.rows_affected, 0);
    assert!(r.last_insert_id.is_none());
    assert!(r.row_count.is_none());
}

#[test]
fn test_savepoint_fields() {
    let sp = Savepoint { id: 42, name: Some("sp1".into()) };
    assert_eq!(sp.id, 42);
    assert_eq!(sp.name.as_deref(), Some("sp1"));
}

#[test]
fn test_conn_state_defaults() {
    let s = ConnState::default();
    assert!(s.auto_commit);
    assert!(!s.read_only);
    assert_eq!(s.transaction_isolation, 2);
    assert!(s.catalog.is_none());
    assert!(s.schema.is_none());
}

#[test]
fn test_wrapper() {
    struct W; impl Wrapper for W {}
    assert!(!W.is_wrapper_for("anything"));
}

#[test]
fn test_row_ops() {
    let r = Row::new(vec![Value::Int(1), Value::String("a".into())]);
    assert_eq!(r.len(), 2); assert!(!r.is_empty());
    assert_eq!(r.get(0), Some(&Value::Int(1)));
    assert!(r.get(2).is_none());
}
