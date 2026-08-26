//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Differential tests: druid-rust vs Druid Java 1.2.28.
use druid::core::*;
use std::sync::Arc;
use std::time::Duration;

// ── PoolConfig defaults ──

// ── ConnectionHolder state machine ──

// ── ExceptionSorter ──

// ── Value display ──

// ── Connection transaction semantics ──

// ── Driver trait ──

// ── ValidConnectionChecker ──

// ── Error Display ──

// ── PoolConfig Builder ──

// ── ConnectionExt defaults ──

// ── ExtendedFilter all default hooks ──

// ── BeforeFilter all default hooks ──

// ── AfterFilter default ──

// ── PoolConfig defaults ──

// ── Value conversions ──

// ── ExecResult all fields ──

#[tokio::test]
async fn task6_differential_tests_test_before_filter_all_default_hooks() {
    struct M;
    #[async_trait::async_trait]
    impl BeforeFilter for M {
        fn name(&self) -> &'static str {
            "m"
        }
        async fn before(&self, _: &mut ExecContext<'_>) -> Result<(), DruidError> {
            Ok(())
        }
    }
    let f = M;
    assert!(f
        .on_connection_event(&ConnectionEvent::Connect)
        .await
        .is_ok());
    assert!(f.on_connection_event(&ConnectionEvent::Close).await.is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::Commit)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::Rollback)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::SetAutoCommit(true))
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::GetAutoCommit)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::SetReadOnly(true))
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::GetReadOnly)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::SetCatalog("db".into()))
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::GetCatalog)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::SetTransactionIsolation(2))
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::GetTransactionIsolation)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::ClearWarnings)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::SetSchema("s".into()))
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::GetSchema)
        .await
        .is_ok());
    assert!(f.on_connection_event(&ConnectionEvent::Abort).await.is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::IsValid)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::NativeSQL("x".into()))
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::SetNetworkTimeout(
            std::time::Duration::from_secs(1)
        ))
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::GetNetworkTimeout)
        .await
        .is_ok());
    assert!(f
        .on_statement_event(&StatementEvent::CreateStatement)
        .await
        .is_ok());
    assert!(f
        .on_statement_event(&StatementEvent::Execute("x".into()))
        .await
        .is_ok());
    assert!(f.on_statement_event(&StatementEvent::Close).await.is_ok());
    assert!(f.on_result_set_event(&ResultSetEvent::Next).await.is_ok());
    assert!(f.on_result_set_event(&ResultSetEvent::Close).await.is_ok());
    assert!(f.init().await.is_ok());
    assert!(f.destroy().await.is_ok());
}

#[tokio::test]
async fn task6_differential_tests_test_after_filter_default() {
    struct M;
    #[async_trait::async_trait]
    impl AfterFilter for M {
        fn name(&self) -> &'static str {
            "m"
        }
        async fn after(
            &self,
            _: &ExecContext<'_>,
            _: &Result<ExecResult, DruidError>,
            _: Duration,
        ) -> Result<(), DruidError> {
            Ok(())
        }
    }
    M.after_connection_event(&ConnectionEvent::Commit, Duration::ZERO)
        .await
        .unwrap();
}

#[test]
fn task6_differential_tests_test_pool_config_builder_all_fields() {
    let c = PoolConfig::builder()
        .name("t")
        .url("postgres://localhost")
        .driver_name("pg")
        .username("u")
        .password("p")
        .max_open(20)
        .min_idle(5)
        .initial_size(10)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .max_lifetime(std::time::Duration::from_mins(10))
        .eviction_interval(std::time::Duration::from_secs(30))
        .min_evictable_idle(std::time::Duration::from_mins(1))
        .test_on_borrow(true)
        .test_on_return(true)
        .validation_query("SELECT 1")
        .keep_alive(true)
        .leak_detection(true)
        .leak_threshold(std::time::Duration::from_mins(1))
        .pool_prepared_statements(true)
        .default_auto_commit(false)
        .break_after_acquire_failure(true)
        .connection_error_retry_attempts(3)
        .async_close_connection(true)
        .slow_sql_threshold(std::time::Duration::from_millis(500))
        .build();
    assert_eq!(c.name, "t");
    assert_eq!(c.max_open, 20);
    assert_eq!(c.min_idle, 5);
    assert!(c.test_on_borrow);
    assert!(c.test_on_return);
    assert!(c.validation_query.is_some());
    assert!(c.keep_alive);
    assert!(c.leak_detection);
    assert!(c.pool_prepared_statements);
    assert_eq!(c.default_auto_commit, Some(false));
    assert!(c.break_after_acquire_failure);
    assert_eq!(c.connection_error_retry_attempts, 3);
    assert!(c.async_close_connection);
}

#[test]
fn task6_differential_tests_test_value_all_from_conversions() {
    let v: Value = true.into();
    assert_eq!(v, Value::Bool(true));
    let v: Value = 42i64.into();
    assert_eq!(v, Value::Int(42));
    let v: Value = 42i32.into();
    assert_eq!(v, Value::Int(42));
    let v: Value = 3.125f64.into();
    assert_eq!(v, Value::Float(3.125));
    let v: Value = String::from("hi").into();
    assert_eq!(v, Value::String("hi".into()));
    let v: Value = "hi".into();
    assert_eq!(v, Value::String("hi".into()));
    let v: Value = vec![1u8, 2u8].into();
    assert_eq!(v, Value::Bytes(vec![1, 2]));
}

#[test]
fn task6_differential_tests_test_exec_result_all_fields() {
    let r = ExecResult::default();
    assert_eq!(r.rows_affected, 0);
    assert!(r.last_insert_id.is_none());
    assert!(r.row_count.is_none());
    let r2 = ExecResult {
        rows_affected: 5,
        last_insert_id: Some(10),
        row_count: Some(100),
    };
    assert_eq!(r2.rows_affected, 5);
    assert_eq!(r2.last_insert_id, Some(10));
    assert_eq!(r2.row_count, Some(100));
}
