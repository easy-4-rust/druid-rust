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

#[test]
fn task6_differential_tests_test_pool_config_defaults_match_druid_java() {
    let c = PoolConfig::default();
    assert_eq!(c.initial_size, 0);
    assert_eq!(c.max_open, 8);
    assert_eq!(c.min_idle, 0);
    assert_eq!(c.acquire_timeout, std::time::Duration::MAX);
    assert_eq!(c.min_evictable_idle, std::time::Duration::from_mins(30));
    assert_eq!(c.eviction_interval, std::time::Duration::from_mins(1));
    assert!(!c.test_on_borrow);
    assert!(!c.test_on_return);
    assert!(!c.pool_prepared_statements);
    assert!(!c.keep_alive);
    assert!(!c.leak_detection);
    assert_eq!(c.leak_threshold, std::time::Duration::from_mins(5));
    assert!(c.use_unfair_lock);
    assert!(!c.break_after_acquire_failure);
    assert_eq!(c.connection_error_retry_attempts, 1);
}

#[test]
fn task6_differential_tests_test_connection_holder_initial_state() {
    assert_eq!(ConnectionHolder::new(1).state(), ConnectionState::Idle);
}

#[test]
fn task6_differential_tests_test_connection_holder_idle_to_active() {
    let h = ConnectionHolder::new(1);
    assert!(h.mark_active());
    assert_eq!(h.state(), ConnectionState::Active);
}

#[test]
fn task6_differential_tests_test_connection_holder_active_to_idle() {
    let h = ConnectionHolder::new(1);
    h.mark_active();
    assert!(h.mark_idle());
    assert_eq!(h.state(), ConnectionState::Idle);
}

#[test]
fn task6_differential_tests_test_connection_holder_cas() {
    let h = ConnectionHolder::new(1);
    assert!(h.try_transition(ConnectionState::Idle, ConnectionState::Idle));
    assert!(h.try_transition(ConnectionState::Idle, ConnectionState::Active));
}

#[test]
fn task6_differential_tests_test_connection_holder_is_alive() {
    assert!(ConnectionHolder::new(1).is_alive(std::time::Duration::from_mins(1)));
}

#[test]
fn task6_differential_tests_test_connection_holder_use_count() {
    let h = ConnectionHolder::new(1);
    h.mark_active();
    h.mark_idle();
    h.mark_active();
    h.mark_idle();
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 2);
}

#[test]
fn task6_differential_tests_test_pg_sorter() {
    assert!(PgExceptionSorter
        .is_exception_fatal(&SqlException::driver(0, "admin shutdown").with_sql_state("08006")));
}

#[test]
fn task6_differential_tests_test_pg_sorter_non_fatal() {
    assert!(!PgExceptionSorter
        .is_exception_fatal(&SqlException::driver(0, "syntax error").with_sql_state("42601")));
}

#[test]
fn task6_differential_tests_test_mysql_sorter() {
    assert!(
        MySqlExceptionSorter.is_exception_fatal(&SqlException::driver(1042, "Can't get hostname"))
    );
}

#[test]
fn task6_differential_tests_test_null_sorter() {
    assert!(!NullExceptionSorter.is_exception_fatal(&SqlException::driver(99999, "anything")));
}

#[test]
fn task6_differential_tests_test_value_display_all() {
    assert_eq!(format!("{}", Value::Null), "NULL");
    assert_eq!(format!("{}", Value::Bool(true)), "true");
    assert_eq!(format!("{}", Value::Int(42)), "42");
    assert_eq!(format!("{}", Value::Float(3.125)), "3.125");
    assert_eq!(format!("{}", Value::String("hello".into())), "'hello'");
    assert_eq!(format!("{}", Value::Bytes(vec![1, 2, 3])), "<3 bytes>");
}

#[tokio::test]
async fn task6_differential_tests_test_begin_commit() {
    struct M {
        tx: bool,
    }
    druid_rdbc::impl_standard_wrapper!(M => Connection);

    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, RdbcError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
            Ok(vec![])
        }
        async fn begin(&mut self) -> Result<(), RdbcError> {
            self.tx = true;
            Ok(())
        }
        async fn commit(&mut self) -> Result<(), RdbcError> {
            self.tx = false;
            Ok(())
        }
        async fn rollback(&mut self) -> Result<(), RdbcError> {
            self.tx = false;
            Ok(())
        }
        async fn ping(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }
        async fn close(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }
    }
    let mut m = M { tx: false };
    m.begin().await.unwrap();
    assert!(m.tx);
    m.commit().await.unwrap();
    assert!(!m.tx);
}

#[tokio::test]
async fn task6_differential_tests_test_rollback() {
    struct M;
    druid_rdbc::impl_standard_wrapper!(M => Connection);

    #[async_trait::async_trait]
    impl Connection for M {
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
    let mut m = M;
    m.begin().await.unwrap();
    m.rollback().await.unwrap();
}

#[tokio::test]
async fn task6_differential_tests_test_savepoint_not_supported() {
    struct M;
    druid_rdbc::impl_standard_wrapper!(M => Connection);

    #[async_trait::async_trait]
    impl Connection for M {
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
    let mut m = M;
    assert!(m.set_savepoint().await.is_err());
    assert!(m.set_savepoint_named("sp1").await.is_err());
    assert!(m
        .release_savepoint(&Savepoint { id: 1, name: None })
        .await
        .is_err());
    assert!(m
        .rollback_to(&Savepoint { id: 1, name: None })
        .await
        .is_err());
}

#[tokio::test]
async fn task6_differential_tests_test_abort_closes() {
    struct M {
        closed: bool,
    }
    druid_rdbc::impl_standard_wrapper!(M => Connection);

    #[async_trait::async_trait]
    impl Connection for M {
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
            self.closed = true;
            Ok(())
        }
    }
    let mut m = M { closed: false };
    m.abort().await.unwrap();
    assert!(m.closed);
}

#[test]
fn task6_differential_tests_test_connection_defaults() {
    struct M;
    druid_rdbc::impl_standard_wrapper!(M => Connection);

    #[async_trait::async_trait]
    impl Connection for M {
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
    let m = M;
    assert!(m.auto_commit().is_err());
    assert!(m.read_only().is_err());
    assert!(m.transaction_isolation().is_err());
    assert!(m.catalog().is_err());
    assert!(m.schema().is_err());
    assert!(m.is_closed().is_err());
    assert!(m.driver_name().is_err());
}

#[test]
fn task6_differential_tests_test_exec_result_default() {
    let r = ExecResult::default();
    assert_eq!(r.rows_affected, 0);
    assert!(r.last_insert_id.is_none());
    assert!(r.row_count.is_none());
}

#[test]
fn task6_differential_tests_test_savepoint_fields() {
    let sp = Savepoint {
        id: 42,
        name: Some("sp1".into()),
    };
    assert_eq!(sp.id, 42);
    assert_eq!(sp.name.as_deref(), Some("sp1"));
}

#[test]
fn task6_differential_tests_test_conn_state_defaults() {
    let s = ConnState::default();
    assert!(s.auto_commit);
    assert!(!s.read_only);
    assert_eq!(s.transaction_isolation, 2);
    assert!(s.catalog.is_none());
    assert!(s.schema.is_none());
}

#[test]
fn task6_differential_tests_test_wrapper() {
    struct W;
    druid_rdbc::impl_standard_wrapper!(W => Wrapper);
    assert!(matches!(
        Wrapper::is_wrapper_for(&W, None),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert_eq!(
        Wrapper::is_wrapper_for(&W, Some(std::any::TypeId::of::<dyn Wrapper>())),
        Ok(true)
    );
    assert!(matches!(
        Wrapper::unwrap(&W, Some(std::any::TypeId::of::<dyn Wrapper>())),
        Ok(StandardUnwrapped::Wrapper(_))
    ));
}

#[test]
fn task6_differential_tests_test_row_ops() {
    let r = Row::new(vec![Value::Int(1), Value::String("a".into())]);
    assert_eq!(r.len(), 2);
    assert!(!r.is_empty());
    assert_eq!(r.get(0), Some(&Value::Int(1)));
    assert!(r.get(2).is_none());
}

#[tokio::test]
async fn task6_differential_tests_test_driver_connect() {
    struct MockDriver;
    #[async_trait::async_trait]
    impl Driver for MockDriver {
        fn name(&self) -> &'static str {
            "test-db"
        }
        fn accepts_url(&self, url: &str) -> bool {
            url.starts_with("postgres://")
        }
        async fn connect(
            &self,
            url: &str,
            _properties: &druid_rdbc::driver::ConnectionProperties,
        ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
            if !self.accepts_url(url) {
                return Ok(None);
            }
            struct C;
            druid_rdbc::impl_standard_wrapper!(C => Connection);
            #[async_trait::async_trait]
            impl Connection for C {
                async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, RdbcError> {
                    Ok(ExecResult::default())
                }
                async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
                    Ok(Vec::new())
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
            Ok(Some(Box::new(C)))
        }
    }
    let driver = MockDriver;
    assert_eq!(driver.name(), "test-db");
    assert!(driver.accepts_url("postgres://localhost"));
    assert!(!driver.accepts_url("mysql://localhost"));
    let mut connection = driver
        .connect(
            "postgres://localhost",
            &druid_rdbc::driver::ConnectionProperties::new(),
        )
        .await
        .unwrap()
        .unwrap();
    connection.ping().await.unwrap();
    assert_eq!(
        connection
            .exec("UPDATE t SET value = 1", Vec::new())
            .await
            .unwrap(),
        ExecResult::default()
    );
    assert!(connection
        .fetch("SELECT 1", Vec::new())
        .await
        .unwrap()
        .is_empty());
    connection.begin().await.unwrap();
    connection.commit().await.unwrap();
    connection.begin().await.unwrap();
    connection.rollback().await.unwrap();
}

#[tokio::test]
async fn task6_differential_tests_test_driver_connect_with_auth_default() {
    struct MockDriver;
    #[async_trait::async_trait]
    impl Driver for MockDriver {
        fn name(&self) -> &'static str {
            "mock"
        }
        fn accepts_url(&self, url: &str) -> bool {
            url.starts_with("postgres://")
        }
        async fn connect(
            &self,
            url: &str,
            properties: &druid_rdbc::driver::ConnectionProperties,
        ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
            if !self.accepts_url(url) {
                return Ok(None);
            }
            struct C(String);
            druid_rdbc::impl_standard_wrapper!(C => Connection);
            #[async_trait::async_trait]
            impl Connection for C {
                async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, RdbcError> {
                    Ok(ExecResult::default())
                }
                async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
                    Ok(Vec::new())
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
                    Ok(self.0.clone())
                }
            }
            let user = properties.get("user").cloned().unwrap_or_default();
            Ok(Some(Box::new(C(format!("{url}#{user}")))))
        }
    }
    let driver = MockDriver;
    let mut properties = druid_rdbc::driver::ConnectionProperties::new();
    properties.insert("user".to_owned(), "user".to_owned());
    properties.insert("password".to_owned(), "pass".to_owned());
    let connection = driver
        .connect("postgres://localhost", &properties)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        connection.driver_name().unwrap(),
        "postgres://localhost#user"
    );
}

#[tokio::test]
async fn task6_differential_tests_test_ping_connection_checker() {
    struct C;
    druid_rdbc::impl_standard_wrapper!(C => Connection);

    #[async_trait::async_trait]
    impl Connection for C {
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
        fn is_closed(&self) -> Result<bool, RdbcError> {
            Ok(false)
        }
        async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
            self.ping().await?;
            Ok(true)
        }
    }
    let checker = PingConnectionChecker;
    let mut conn = Box::new(C) as Box<dyn Connection>;
    assert!(checker.is_valid(&mut conn).await);
}

#[test]
fn task6_differential_tests_test_error_display_all_variants() {
    let cases = vec![
        (
            RdbcError::DataSourceClosed {
                close_time_millis: 0,
            },
            "dataSource already closed at ",
        ),
        (RdbcError::AcquireTimeout, "acquire connection timed out"),
        (RdbcError::PoolExhausted, "connection pool exhausted"),
        (
            RdbcError::ValidationFailed("test".into()),
            "validation failed: test",
        ),
        (
            RdbcError::ConnectionLeaked {
                id: 42,
                held_for: std::time::Duration::from_secs(10),
            },
            "connection 42 leaked",
        ),
        (
            RdbcError::ConnectionDiscarded,
            "connection has been discarded",
        ),
        (
            RdbcError::DriverError("timeout".into()),
            "driver error: timeout",
        ),
        (
            RdbcError::SqlParseError("syntax".into()),
            "SQL parse error: syntax",
        ),
        (
            RdbcError::WallViolation("DROP".into()),
            "wall violation: DROP",
        ),
        (
            RdbcError::DataSourceNotFound("test".into()),
            "datasource not found: test",
        ),
        (RdbcError::Other("misc".into()), "misc"),
    ];
    for (err, expected) in cases {
        assert!(format!("{err}").contains(expected));
    }
}

#[test]
fn task6_differential_tests_test_pool_config_builder_every_field() {
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

#[tokio::test]
async fn task6_differential_tests_test_connection_ext_all_defaults() {
    struct M;
    druid_rdbc::impl_standard_wrapper!(M => Connection);
    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, RdbcError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
            Ok(Vec::new())
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
        async fn create_statement(
            &mut self,
            _: druid_rdbc::statement::StatementOptions,
        ) -> Result<Box<dyn Statement>, RdbcError> {
            Err(RdbcError::Other("n/a".into()))
        }
        async fn prepare_statement(
            &mut self,
            _: &str,
            _: druid_rdbc::statement::StatementOptions,
        ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
            Err(RdbcError::Other("n/a".into()))
        }
        async fn prepare_call(
            &mut self,
            _: &str,
            _: druid_rdbc::statement::StatementOptions,
        ) -> Result<Box<dyn druid_rdbc::statement::CallableStatement>, RdbcError> {
            Err(RdbcError::Other("n/a".into()))
        }
        async fn database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
            Ok(None)
        }
        async fn database_product_version(&mut self) -> Result<Option<String>, RdbcError> {
            Ok(None)
        }
        fn driver_major_version(&self) -> Result<i32, RdbcError> {
            Ok(0)
        }
        fn driver_minor_version(&self) -> Result<i32, RdbcError> {
            Ok(0)
        }
        fn holdability(&self) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
            Ok(druid_rdbc::statement::ResultSetHoldability::CloseAtCommit)
        }
        async fn set_holdability(
            &mut self,
            _: druid_rdbc::statement::ResultSetHoldability,
        ) -> Result<(), RdbcError> {
            Ok(())
        }
        async fn set_client_info(&mut self, _: &str, _: Option<&str>) -> Result<(), RdbcError> {
            Ok(())
        }
        fn client_info(&self, _: &str) -> Result<Option<String>, RdbcError> {
            Ok(None)
        }
        async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }
        async fn native_sql(&mut self, sql: &str) -> Result<String, RdbcError> {
            Ok(sql.to_owned())
        }
        async fn set_network_timeout(&mut self, _: Duration) -> Result<(), RdbcError> {
            Ok(())
        }
        fn network_timeout(&self) -> Result<Duration, RdbcError> {
            Ok(Duration::ZERO)
        }
        fn type_map(&self) -> Result<Option<druid_rdbc::types::RdbcTypeMap>, RdbcError> {
            Ok(None)
        }
        async fn set_type_map(
            &mut self,
            _: druid_rdbc::types::RdbcTypeMap,
        ) -> Result<(), RdbcError> {
            Ok(())
        }
    }
    let mut c = M;
    let options = druid_rdbc::statement::StatementOptions::default();
    assert!(c.create_statement(options).await.is_err());
    assert!(c.prepare_statement("x", options).await.is_err());
    assert!(c.prepare_call("x", options).await.is_err());
    assert!(c.database_meta_data().await.is_err());
    assert_eq!(c.database_product_name().await.unwrap(), None);
    assert_eq!(c.database_product_version().await.unwrap(), None);
    assert_eq!(c.driver_major_version(), Ok(0));
    assert_eq!(c.driver_minor_version(), Ok(0));
    assert_eq!(
        c.holdability(),
        Ok(druid_rdbc::statement::ResultSetHoldability::CloseAtCommit)
    );
    Connection::set_holdability(
        &mut c,
        druid_rdbc::statement::ResultSetHoldability::CloseAtCommit,
    )
    .await
    .unwrap();
    c.set_client_info("k", Some("v")).await.unwrap();
    assert_eq!(c.client_info("k").unwrap(), None);
    Connection::clear_warnings(&mut c).await.unwrap();
    assert_eq!(c.native_sql("SELECT 1").await.unwrap(), "SELECT 1");
    c.set_network_timeout(Duration::from_secs(10))
        .await
        .unwrap();
    assert_eq!(c.network_timeout(), Ok(Duration::ZERO));
    assert_eq!(c.type_map().unwrap(), None);
    c.set_type_map(druid_rdbc::types::RdbcTypeMap::new())
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_differential_tests_test_extended_filter_all_default_hooks() {
    struct M;
    #[async_trait::async_trait]
    impl ExtendedFilter for M {
        async fn on_statement_property_event(
            &self,
            _: &StatementPropertyEvent,
        ) -> Result<(), DruidError> {
            Ok(())
        }
        async fn on_clob_event(&self, _: &ClobEvent) -> Result<(), DruidError> {
            Ok(())
        }
        async fn on_datasource_event(&self, _: &DataSourceEvent) -> Result<(), DruidError> {
            Ok(())
        }
    }
    let f = M;
    assert!(f
        .on_statement_property_event(&StatementPropertyEvent::SetQueryTimeout(100))
        .await
        .is_ok());
    assert!(f.on_clob_event(&ClobEvent::Length).await.is_ok());
    assert!(f
        .on_datasource_event(&DataSourceEvent::GetConnection)
        .await
        .is_ok());
    assert!(!f.is_wrapper_for("any"));
}
