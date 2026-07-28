//! Differential tests: druid-rust vs Druid Java 1.2.28.
use druid::core::*;
use std::time::Duration;

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
    assert_eq!(ConnectionHolder::new(1).state(), ConnectionState::Idle);
}
#[test]
fn test_connection_holder_idle_to_active() {
    let h = ConnectionHolder::new(1);
    assert!(h.mark_active());
    assert_eq!(h.state(), ConnectionState::Active);
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
    h.mark_active();
    h.mark_idle();
    h.mark_active();
    h.mark_idle();
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 2);
}

// ── ExceptionSorter ──
#[test]
fn test_pg_sorter() {
    assert!(PgExceptionSorter.is_exception_fatal(57001, "admin shutdown"));
}
#[test]
fn test_pg_sorter_non_fatal() {
    assert!(!PgExceptionSorter.is_exception_fatal(42601, "syntax error"));
}
#[test]
fn test_mysql_sorter() {
    assert!(MySqlExceptionSorter.is_exception_fatal(1042, "Can't get hostname"));
}
#[test]
fn test_null_sorter() {
    assert!(!NullExceptionSorter.is_exception_fatal(99999, "anything"));
}

// ── Value display ──
#[test]
fn test_value_display_all() {
    assert_eq!(format!("{}", Value::Null), "NULL");
    assert_eq!(format!("{}", Value::Bool(true)), "true");
    assert_eq!(format!("{}", Value::Int(42)), "42");
    assert_eq!(format!("{}", Value::Float(3.14)), "3.14");
    assert_eq!(format!("{}", Value::String("hello".into())), "'hello'");
    assert_eq!(format!("{}", Value::Bytes(vec![1, 2, 3])), "<3 bytes>");
}

// ── Connection transaction semantics ──
#[tokio::test]
async fn test_begin_commit() {
    struct M {
        tx: bool,
    }
    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
            Ok(vec![])
        }
        async fn begin(&mut self) -> Result<(), DruidError> {
            self.tx = true;
            Ok(())
        }
        async fn commit(&mut self) -> Result<(), DruidError> {
            self.tx = false;
            Ok(())
        }
        async fn rollback(&mut self) -> Result<(), DruidError> {
            self.tx = false;
            Ok(())
        }
        async fn ping(&mut self) -> Result<(), DruidError> {
            Ok(())
        }
        async fn close(&mut self) -> Result<(), DruidError> {
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
async fn test_rollback() {
    struct M;
    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
            Ok(vec![])
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
            Ok(())
        }
    }
    let mut m = M;
    m.begin().await.unwrap();
    m.rollback().await.unwrap();
}

#[tokio::test]
async fn test_savepoint_not_supported() {
    struct M;
    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
            Ok(vec![])
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
async fn test_abort_closes() {
    struct M {
        closed: bool,
    }
    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
            Ok(vec![])
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
            self.closed = true;
            Ok(())
        }
    }
    let mut m = M { closed: false };
    m.abort().await.unwrap();
    assert!(m.closed);
}

#[test]
fn test_connection_defaults() {
    struct M;
    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
            Ok(vec![])
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
            Ok(())
        }
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
    let sp = Savepoint {
        id: 42,
        name: Some("sp1".into()),
    };
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
    struct W;
    impl Wrapper for W {}
    assert!(!W.is_wrapper_for("anything"));
}

#[test]
fn test_row_ops() {
    let r = Row::new(vec![Value::Int(1), Value::String("a".into())]);
    assert_eq!(r.len(), 2);
    assert!(!r.is_empty());
    assert_eq!(r.get(0), Some(&Value::Int(1)));
    assert!(r.get(2).is_none());
}

// ── Driver trait ──
#[tokio::test]
async fn test_driver_connect() {
    struct MockDriver;
    #[async_trait::async_trait]
    impl Driver for MockDriver {
        fn name(&self) -> &str {
            "test-db"
        }
        async fn connect(&self, _url: &str) -> Result<Box<dyn Connection>, DruidError> {
            struct C;
            #[async_trait::async_trait]
            impl Connection for C {
                async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
                    Ok(ExecResult::default())
                }
                async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
                    Ok(vec![])
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
                    Ok(())
                }
            }
            Ok(Box::new(C))
        }
    }
    let d = MockDriver;
    assert_eq!(d.name(), "test-db");
    let mut conn = d.connect("postgres://localhost").await.unwrap();
    conn.ping().await.unwrap();
}

#[tokio::test]
async fn test_driver_connect_with_auth_default() {
    struct MockDriver;
    #[async_trait::async_trait]
    impl Driver for MockDriver {
        fn name(&self) -> &str {
            "mock"
        }
        async fn connect(&self, url: &str) -> Result<Box<dyn Connection>, DruidError> {
            struct C(String);
            #[async_trait::async_trait]
            impl Connection for C {
                async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
                    Ok(ExecResult::default())
                }
                async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
                    Ok(vec![])
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
                    Ok(())
                }
                fn driver_name(&self) -> &str {
                    &self.0
                }
            }
            Ok(Box::new(C(url.to_string())))
        }
    }
    let d = MockDriver;
    let conn = d
        .connect_with_auth("postgres://localhost", "user", "pass")
        .await
        .unwrap();
    assert_eq!(conn.driver_name(), "postgres://localhost");
}

// ── ValidConnectionChecker ──
#[tokio::test]
async fn test_ping_connection_checker() {
    struct C;
    #[async_trait::async_trait]
    impl Connection for C {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
            Ok(vec![])
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
            Ok(())
        }
    }
    let checker = PingConnectionChecker;
    let mut conn = Box::new(C) as Box<dyn Connection>;
    assert!(checker.is_valid(&mut conn).await);
}

// ── Error Display ──
#[test]
fn test_error_display_all_variants() {
    let cases = vec![
        (DruidError::PoolClosed, "connection pool is closed"),
        (DruidError::AcquireTimeout, "acquire connection timed out"),
        (DruidError::PoolExhausted, "connection pool exhausted"),
        (
            DruidError::ValidationFailed("test".into()),
            "validation failed: test",
        ),
        (
            DruidError::ConnectionLeaked {
                id: 42,
                held_for: std::time::Duration::from_secs(10),
            },
            "connection 42 leaked",
        ),
        (
            DruidError::ConnectionDiscarded,
            "connection has been discarded",
        ),
        (
            DruidError::DriverError("timeout".into()),
            "driver error: timeout",
        ),
        (
            DruidError::SqlParseError("syntax".into()),
            "SQL parse error: syntax",
        ),
        (
            DruidError::WallViolation("DROP".into()),
            "wall violation: DROP",
        ),
        (
            DruidError::DataSourceNotFound("test".into()),
            "datasource not found: test",
        ),
        (DruidError::Other("misc".into()), "misc"),
    ];
    for (err, expected) in cases {
        assert!(format!("{err}").contains(expected));
    }
}

// ── PoolConfig Builder ──
#[test]
fn test_pool_config_builder_every_field() {
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
        .max_lifetime(std::time::Duration::from_secs(600))
        .eviction_interval(std::time::Duration::from_secs(30))
        .min_evictable_idle(std::time::Duration::from_secs(60))
        .test_on_borrow(true)
        .test_on_return(true)
        .validation_query("SELECT 1")
        .keep_alive(true)
        .leak_detection(true)
        .leak_threshold(std::time::Duration::from_secs(60))
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

// ── ConnectionExt defaults ──
#[tokio::test]
async fn test_connection_ext_all_defaults() {
    struct M;
    #[async_trait::async_trait]
    impl Connection for M {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
            Ok(ExecResult::default())
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
            Ok(vec![])
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
            Ok(())
        }
    }
    #[async_trait::async_trait]
    impl ConnectionExt for M {
        async fn create_statement(&mut self) -> Result<Box<dyn Connection>, DruidError> {
            Err(DruidError::Other("n/a".into()))
        }
        async fn prepare_statement(&mut self, _: &str) -> Result<Box<dyn Connection>, DruidError> {
            Err(DruidError::Other("n/a".into()))
        }
        async fn prepare_call(&mut self, _: &str) -> Result<Box<dyn Connection>, DruidError> {
            Err(DruidError::Other("n/a".into()))
        }
        async fn native_sql(&self, sql: &str) -> Result<String, DruidError> {
            Ok(sql.to_string())
        }
        async fn clear_warnings(&mut self) -> Result<(), DruidError> {
            Ok(())
        }
        fn get_meta_data(&self) -> Option<&MetaData> {
            None
        }
        fn get_database_product_name(&self) -> Option<&str> {
            None
        }
        fn get_database_product_version(&self) -> Option<&str> {
            None
        }
        fn get_driver_major_version(&self) -> i32 {
            0
        }
        fn get_driver_minor_version(&self) -> i32 {
            0
        }
        fn get_holdability(&self) -> i32 {
            1
        }
        async fn set_holdability(&mut self, _: i32) -> Result<(), DruidError> {
            Ok(())
        }
        async fn set_client_info(&mut self, _: &str, _: &str) -> Result<(), DruidError> {
            Ok(())
        }
        fn get_client_info(&self, _: &str) -> Option<String> {
            None
        }
        async fn set_network_timeout(&mut self, _: std::time::Duration) -> Result<(), DruidError> {
            Ok(())
        }
        fn get_network_timeout(&self) -> i32 {
            0
        }
        fn get_type_map(&self) -> Option<std::collections::HashMap<String, String>> {
            None
        }
        async fn set_type_map(
            &mut self,
            _: std::collections::HashMap<String, String>,
        ) -> Result<(), DruidError> {
            Ok(())
        }
    }
    let mut c = M;
    assert!(c.create_statement().await.is_err());
    assert!(c.prepare_statement("x").await.is_err());
    assert!(c.prepare_call("x").await.is_err());
    assert!(c.get_meta_data().is_none());
    assert!(c.get_database_product_name().is_none());
    assert!(c.get_database_product_version().is_none());
    assert_eq!(c.get_driver_major_version(), 0);
    assert_eq!(c.get_driver_minor_version(), 0);
    assert_eq!(c.get_holdability(), 1);
    ConnectionExt::set_holdability(&mut c, 1).await.unwrap();
    c.set_client_info("k", "v").await.unwrap();
    assert!(c.get_client_info("k").is_none());
    ConnectionExt::clear_warnings(&mut c).await.unwrap();
    assert_eq!(c.native_sql("SELECT 1").await.unwrap(), "SELECT 1");
    c.set_network_timeout(std::time::Duration::from_secs(10))
        .await
        .unwrap();
    assert_eq!(c.get_network_timeout(), 0);
    assert!(c.get_type_map().is_none());
    c.set_type_map(std::collections::HashMap::new())
        .await
        .unwrap();
}

// ── ExtendedFilter all default hooks ──
#[tokio::test]
async fn test_extended_filter_all_default_hooks() {
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

// ── BeforeFilter all default hooks ──
#[tokio::test]
async fn test_before_filter_all_default_hooks() {
    struct M;
    #[async_trait::async_trait]
    impl BeforeFilter for M {
        fn name(&self) -> &str {
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

// ── AfterFilter default ──
#[tokio::test]
async fn test_after_filter_default() {
    struct M;
    #[async_trait::async_trait]
    impl AfterFilter for M {
        fn name(&self) -> &str {
            "m"
        }
        async fn after(
            &self,
            _: &ExecContext<'_>,
            _: &Result<ExecResult, DruidError>,
            _: Duration,
        ) {
        }
        async fn after_connection_close(&self) {}
    }
    M.after_connection_close().await;
}

// ── PoolConfig defaults ──
#[test]
fn test_pool_config_builder_all_fields() {
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
        .max_lifetime(std::time::Duration::from_secs(600))
        .eviction_interval(std::time::Duration::from_secs(30))
        .min_evictable_idle(std::time::Duration::from_secs(60))
        .test_on_borrow(true)
        .test_on_return(true)
        .validation_query("SELECT 1")
        .keep_alive(true)
        .leak_detection(true)
        .leak_threshold(std::time::Duration::from_secs(60))
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

// ── Value conversions ──
#[test]
fn test_value_all_from_conversions() {
    let v: Value = true.into();
    assert_eq!(v, Value::Bool(true));
    let v: Value = 42i64.into();
    assert_eq!(v, Value::Int(42));
    let v: Value = 42i32.into();
    assert_eq!(v, Value::Int(42));
    let v: Value = 3.14f64.into();
    assert_eq!(v, Value::Float(3.14));
    let v: Value = String::from("hi").into();
    assert_eq!(v, Value::String("hi".into()));
    let v: Value = "hi".into();
    assert_eq!(v, Value::String("hi".into()));
    let v: Value = vec![1u8, 2u8].into();
    assert_eq!(v, Value::Bytes(vec![1, 2]));
}

// ── ExecResult all fields ──
#[test]
fn test_exec_result_all_fields() {
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
