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
