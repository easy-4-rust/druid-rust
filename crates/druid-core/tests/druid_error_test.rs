extern crate druid_core as druid;
use druid::core::{DruidError, SqlException};
use std::time::Duration;

// ── Display ────────────────────────────────────────────────────

#[test]
fn error_display_pool_closed() {
    assert_eq!(
        format!("{}", DruidError::PoolClosed),
        "connection pool is closed"
    );
}

#[test]
fn error_display_acquire_timeout() {
    assert_eq!(
        format!("{}", DruidError::AcquireTimeout),
        "acquire connection timed out"
    );
}

#[test]
fn error_display_login_timeout() {
    assert_eq!(
        format!("{}", DruidError::LoginTimeout),
        "physical connection login timed out"
    );
}

#[test]
fn error_display_pool_exhausted() {
    assert_eq!(
        format!("{}", DruidError::PoolExhausted),
        "connection pool exhausted"
    );
}

#[test]
fn error_display_connection_discarded() {
    assert_eq!(
        format!("{}", DruidError::ConnectionDiscarded),
        "connection has been discarded"
    );
}

#[test]
fn error_display_datasource_disabled() {
    assert_eq!(
        format!("{}", DruidError::DataSourceDisabled),
        "datasource is disabled"
    );
}

#[test]
fn error_display_validation_failed() {
    let e = DruidError::ValidationFailed("bad conn".to_owned());
    assert!(format!("{}", e).contains("bad conn"));
}

#[test]
fn error_display_driver_error() {
    let e = DruidError::DriverError("oops".to_owned());
    assert!(format!("{}", e).contains("oops"));
}

#[test]
fn error_display_sql_parse_error() {
    let e = DruidError::SqlParseError("syntax".to_owned());
    assert!(format!("{}", e).contains("syntax"));
}

#[test]
fn error_display_wall_violation() {
    let e = DruidError::WallViolation("blocked".to_owned());
    assert!(format!("{}", e).contains("blocked"));
}

#[test]
fn error_display_datasource_not_found() {
    let e = DruidError::DataSourceNotFound("mydb".to_owned());
    assert!(format!("{}", e).contains("mydb"));
}

#[test]
fn error_display_invalid_argument() {
    let e = DruidError::InvalidArgument("bad param".to_owned());
    assert!(format!("{}", e).contains("bad param"));
}

#[test]
fn error_display_unsupported_operation() {
    let e = DruidError::UnsupportedOperation {
        operation: "test_op",
    };
    assert!(format!("{}", e).contains("test_op"));
}

#[test]
fn error_display_other() {
    let e = DruidError::Other("something".to_owned());
    assert_eq!(format!("{}", e), "something");
}

#[test]
fn error_display_datasource_closed() {
    let e = DruidError::DataSourceClosed {
        close_time_millis: 0,
    };
    assert!(format!("{}", e).contains("closed"));
}

#[test]
fn error_display_max_wait_thread_count() {
    let e = DruidError::MaxWaitThreadCountExceeded {
        max: 10,
        current: 11,
    };
    let s = format!("{}", e);
    assert!(s.contains("10"));
    assert!(s.contains("11"));
}

#[test]
fn error_display_connection_leaked() {
    let e = DruidError::ConnectionLeaked {
        id: 42,
        held_for: Duration::from_secs(30),
    };
    let s = format!("{}", e);
    assert!(s.contains("42"));
}

#[test]
fn error_display_active_connections_prevent_restart() {
    let e = DruidError::ActiveConnectionsPreventRestart { active_count: 5 };
    assert!(format!("{}", e).contains("5"));
}

#[test]
fn error_display_datasource_not_available_none() {
    let e = DruidError::DataSourceNotAvailable { cause: None };
    assert!(format!("{}", e).contains("not available"));
}

#[test]
fn error_display_datasource_not_available_with_cause() {
    let e = DruidError::DataSourceNotAvailable {
        cause: Some(Box::new(DruidError::PoolClosed)),
    };
    assert!(format!("{}", e).contains("closed"));
}

#[test]
fn error_display_sql_exception() {
    let exc = SqlException::new(100, Some("HY000".to_owned()), Some("test error".to_owned()));
    let e = DruidError::SqlException(Box::new(exc));
    let s = format!("{}", e);
    assert!(s.contains("100"));
    assert!(s.contains("test error"));
}

#[test]
fn error_display_batch_update() {
    let e = DruidError::BatchUpdateException {
        update_counts: vec![1, 2, 3],
        cause: Box::new(DruidError::Other("fail".to_owned())),
    };
    let s = format!("{}", e);
    assert!(s.contains("3 result(s)"));
}

// ── class_name ─────────────────────────────────────────────────

#[test]
fn error_class_name_variants() {
    assert_eq!(DruidError::PoolClosed.class_name(), "druid::PoolClosed");
    assert_eq!(
        DruidError::AcquireTimeout.class_name(),
        "druid::AcquireTimeout"
    );
    assert_eq!(DruidError::LoginTimeout.class_name(), "druid::LoginTimeout");
    assert_eq!(
        DruidError::PoolExhausted.class_name(),
        "druid::PoolExhausted"
    );
    assert_eq!(
        DruidError::ConnectionDiscarded.class_name(),
        "druid::ConnectionDiscarded"
    );
    assert_eq!(
        DruidError::DataSourceDisabled.class_name(),
        "com.alibaba.druid.pool.DataSourceDisableException"
    );
    assert_eq!(
        DruidError::InvalidArgument("x".to_owned()).class_name(),
        "druid::InvalidArgument"
    );
    assert_eq!(
        DruidError::Other("x".to_owned()).class_name(),
        "druid::Other"
    );
    assert_eq!(
        DruidError::DriverError("x".to_owned()).class_name(),
        "druid::DriverError"
    );
    assert_eq!(
        DruidError::SqlParseError("x".to_owned()).class_name(),
        "druid::SqlParseError"
    );
    assert_eq!(
        DruidError::WallViolation("x".to_owned()).class_name(),
        "druid::WallViolation"
    );
    assert_eq!(
        DruidError::DataSourceNotFound("x".to_owned()).class_name(),
        "druid::DataSourceNotFound"
    );
}

// ── GetConnectionTimeout Display branches ──────────────────────

#[test]
fn error_display_get_connection_timeout_basic() {
    let e = DruidError::GetConnectionTimeout {
        wait_millis: 5000,
        active_count: 10,
        max_active: 10,
        creating_count: 2,
        create_elapsed_millis: None,
        create_error_count: 0,
        running_sql: vec![],
        cause: None,
    };
    let s = format!("{}", e);
    assert!(s.contains("5000"));
    assert!(s.contains("active 10"));
    assert!(!s.contains("createElapseMillis"));
    assert!(!s.contains("createErrorCount"));
}

#[test]
fn error_display_get_connection_timeout_with_elapsed() {
    let e = DruidError::GetConnectionTimeout {
        wait_millis: 3000,
        active_count: 5,
        max_active: 8,
        creating_count: 1,
        create_elapsed_millis: Some(1500),
        create_error_count: 0,
        running_sql: vec![],
        cause: None,
    };
    let s = format!("{}", e);
    assert!(s.contains("createElapseMillis 1500"));
}

#[test]
fn error_display_get_connection_timeout_with_errors() {
    let e = DruidError::GetConnectionTimeout {
        wait_millis: 3000,
        active_count: 5,
        max_active: 8,
        creating_count: 1,
        create_elapsed_millis: None,
        create_error_count: 3,
        running_sql: vec![],
        cause: None,
    };
    let s = format!("{}", e);
    assert!(s.contains("createErrorCount 3"));
}

#[test]
fn error_display_get_connection_timeout_with_running_sql() {
    let e = DruidError::GetConnectionTimeout {
        wait_millis: 3000,
        active_count: 5,
        max_active: 8,
        creating_count: 1,
        create_elapsed_millis: Some(100),
        create_error_count: 1,
        running_sql: vec![
            (1, "SELECT * FROM t1".to_owned()),
            (2, "INSERT INTO t2 VALUES (1)".to_owned()),
        ],
        cause: None,
    };
    let s = format!("{}", e);
    assert!(s.contains("runningSqlCount 1"));
    assert!(s.contains("SELECT * FROM t1"));
    assert!(s.contains("runningSqlCount 2"));
}

#[test]
fn error_display_get_connection_timeout_zero_elapsed() {
    let e = DruidError::GetConnectionTimeout {
        wait_millis: 3000,
        active_count: 5,
        max_active: 8,
        creating_count: 1,
        create_elapsed_millis: Some(0),
        create_error_count: 0,
        running_sql: vec![],
        cause: None,
    };
    let s = format!("{}", e);
    assert!(!s.contains("createElapseMillis"));
}

// ── OnFatalError Display branches ──────────────────────────────

#[test]
fn error_display_on_fatal_error_basic() {
    let e = DruidError::OnFatalError {
        active_count: 5,
        max_active: 10,
        last_error_time_millis: 0,
        last_sql: None,
        cause: None,
    };
    let s = format!("{}", e);
    assert!(s.contains("activeCount 5"));
    assert!(s.contains("onFatalErrorMaxActive 10"));
    assert!(!s.contains("time"));
    assert!(!s.contains("sql"));
}

#[test]
fn error_display_on_fatal_error_with_time() {
    let e = DruidError::OnFatalError {
        active_count: 3,
        max_active: 10,
        last_error_time_millis: 1700000000000,
        last_sql: None,
        cause: None,
    };
    let s = format!("{}", e);
    assert!(s.contains("time"));
}

#[test]
fn error_display_on_fatal_error_with_sql() {
    use druid::core::RdbcString;
    let e = DruidError::OnFatalError {
        active_count: 3,
        max_active: 10,
        last_error_time_millis: 0,
        last_sql: Some(RdbcString::from_rust_str("SELECT 1")),
        cause: None,
    };
    let s = format!("{}", e);
    assert!(s.contains("sql"));
    assert!(s.contains("SELECT 1"));
}

#[test]
fn error_display_on_fatal_error_zero_time() {
    let e = DruidError::OnFatalError {
        active_count: 3,
        max_active: 10,
        last_error_time_millis: 0,
        last_sql: None,
        cause: None,
    };
    let s = format!("{}", e);
    assert!(!s.contains("time"));
}

// ── DataSourceClosed Display ───────────────────────────────────

#[test]
fn error_display_datasource_closed_valid_time() {
    let e = DruidError::DataSourceClosed {
        close_time_millis: 1700000000000,
    };
    let s = format!("{}", e);
    assert!(s.contains("closed"));
}

#[test]
fn error_display_datasource_closed_zero_time() {
    let e = DruidError::DataSourceClosed {
        close_time_millis: 0,
    };
    let s = format!("{}", e);
    assert!(s.contains("0"));
}

// ── sql_exception / batch_update_counts ────────────────────────

#[test]
fn error_sql_exception_none() {
    assert!(DruidError::PoolClosed.sql_exception().is_none());
}

#[test]
fn error_sql_exception_some() {
    let exc = SqlException::new(1, None, None);
    let e = DruidError::SqlException(Box::new(exc));
    assert!(e.sql_exception().is_some());
}

#[test]
fn error_batch_update_counts_none() {
    assert!(DruidError::PoolClosed.batch_update_counts().is_none());
}

#[test]
fn error_batch_update_counts_some() {
    let e = DruidError::BatchUpdateException {
        update_counts: vec![1, -3, 2],
        cause: Box::new(DruidError::Other("fail".to_owned())),
    };
    assert_eq!(e.batch_update_counts(), Some([1, -3, 2].as_slice()));
}

// ── Error trait source ─────────────────────────────────────────

#[test]
fn error_source_none() {
    use std::error::Error;
    let e = DruidError::PoolClosed;
    assert!(e.source().is_none());
}

#[test]
fn error_source_batch_update() {
    use std::error::Error;
    let e = DruidError::BatchUpdateException {
        update_counts: vec![],
        cause: Box::new(DruidError::Other("cause".to_owned())),
    };
    assert!(e.source().is_some());
}

// ── From impls ─────────────────────────────────────────────────

#[test]
fn error_from_string() {
    let e: DruidError = "test".to_string().into();
    assert_eq!(e, DruidError::Other("test".to_owned()));
}

#[test]
fn error_from_str() {
    let e: DruidError = "test".into();
    assert_eq!(e, DruidError::Other("test".to_owned()));
}

// ── feature_not_supported / rdbc_resource_closed ───────────────

#[test]
fn error_feature_not_supported() {
    let e = DruidError::feature_not_supported("test_op");
    match &e {
        DruidError::SqlException(exc) => {
            assert_eq!(exc.sql_state(), Some("0A000"));
        }
        other => panic!("expected SqlException, got {other:?}"),
    }
}

#[test]
fn error_rdbc_resource_closed() {
    let e = DruidError::rdbc_resource_closed("ResultSet");
    match &e {
        DruidError::SqlException(exc) => {
            assert_eq!(exc.sql_state(), Some("HY010"));
        }
        other => panic!("expected SqlException, got {other:?}"),
    }
}

// ── Clone / Debug / PartialEq ──────────────────────────────────

#[test]
fn error_clone_eq() {
    let e1 = DruidError::PoolClosed;
    let e2 = e1.clone();
    assert_eq!(e1, e2);
}

#[test]
fn error_debug() {
    let e = DruidError::PoolClosed;
    assert!(format!("{:?}", e).contains("PoolClosed"));
}
