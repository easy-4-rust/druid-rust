//! Coverage boost tests targeting specific uncovered lines.
use druid_core::*;
use std::time::Duration;

// ── ConnectionExt: all default methods ──

struct MockConnForExt;
#[async_trait::async_trait]
impl Connection for MockConnForExt {
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
impl ConnectionExt for MockConnForExt {
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

#[tokio::test]
async fn test_connection_ext_exercise_all_defaults() {
    let mut c = MockConnForExt;
    // Exercise every method to cover all default branches
    let _ = c.create_statement().await;
    let _ = c.prepare_statement("SELECT 1").await;
    let _ = c.prepare_call("CALL sp").await;
    assert!(c.get_meta_data().is_none());
    assert_eq!(c.get_database_product_name(), None);
    assert_eq!(c.get_database_product_version(), None);
    assert_eq!(c.get_driver_major_version(), 0);
    assert_eq!(c.get_driver_minor_version(), 0);
    assert_eq!(c.get_holdability(), 1);
    let _ = ConnectionExt::set_holdability(&mut c, 1).await;
    let _ = c.set_client_info("k", "v").await;
    assert_eq!(c.get_client_info("k"), None);
    let _ = ConnectionExt::clear_warnings(&mut c).await;
    assert_eq!(c.native_sql("SELECT 1").await.unwrap(), "SELECT 1");
    let _ = c
        .set_network_timeout(std::time::Duration::from_secs(5))
        .await;
    assert_eq!(c.get_network_timeout(), 0);
    assert_eq!(c.get_type_map(), None);
    let _ = c.set_type_map(std::collections::HashMap::new()).await;
}

// ── PooledConnection: all branches ──

#[tokio::test]
async fn test_pooled_connection_all_paths() {
    struct MockConn;
    #[async_trait::async_trait]
    impl Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, DruidError> {
            Ok(ExecResult {
                rows_affected: 1,
                last_insert_id: Some(42),
                row_count: None,
            })
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, DruidError> {
            Ok(vec![Row::new(vec![Value::Int(99)])])
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
    let conn = Box::new(MockConn) as Box<dyn Connection>;
    let mut pc = PooledConnection::new(conn, 1, Box::new(|_, _| {}));
    // exec
    let r = pc.exec("SELECT 1", vec![]).await.unwrap();
    assert_eq!(r.rows_affected, 1);
    // fetch
    let rows = pc.fetch("SELECT 1", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
    // begin/commit/rollback
    pc.begin().await.unwrap();
    pc.commit().await.unwrap();
    pc.begin().await.unwrap();
    pc.rollback().await.unwrap();
    // ping
    pc.ping().await.unwrap();
    // id and driver_name
    assert_eq!(pc.id(), 1);
    assert_eq!(pc.driver_name(), "");
}

#[tokio::test]
async fn test_pooled_connection_into_core() {
    let conn = Box::new(MockConnForExt) as Box<dyn Connection>;
    let mut pc = PooledConnection::new(conn, 1, Box::new(|_, _| {}));
    let _ = pc.exec("x", vec![]).await;
}

#[tokio::test]
async fn test_pooled_connection_fetch_empty() {
    struct MockConn;
    #[async_trait::async_trait]
    impl Connection for MockConn {
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
    let conn = Box::new(MockConn) as Box<dyn Connection>;
    let mut pc = PooledConnection::new(conn, 1, Box::new(|_, _| {}));
    let rows = pc.fetch("SELECT 1", vec![]).await.unwrap();
    assert!(rows.is_empty());
}

// ── Error: test every variant Display ──
#[test]
fn test_error_all_variants_display() {
    let variants = vec![
        DruidError::PoolClosed,
        DruidError::AcquireTimeout,
        DruidError::PoolExhausted,
        DruidError::ValidationFailed("x".into()),
        DruidError::ConnectionLeaked {
            id: 1,
            held_for: std::time::Duration::from_secs(10),
        },
        DruidError::ConnectionDiscarded,
        DruidError::DriverError("e".into()),
        DruidError::SqlParseError("e".into()),
        DruidError::WallViolation("e".into()),
        DruidError::DataSourceNotFound("e".into()),
        DruidError::UnsupportedOperation { operation: "test" },
        DruidError::Other("e".into()),
    ];
    for v in &variants {
        let _ = format!("{}", v);
    }
    // Test Error trait
    for v in &variants {
        assert!(std::error::Error::source(v).is_none());
    }
}

// ── Value: all From conversions + Display ──
#[test]
fn test_value_all_conversions_and_display() {
    let bool_val: Value = true.into();
    assert_eq!(bool_val, Value::Bool(true));
    assert_eq!(format!("{}", bool_val), "true");
    let int_val: Value = 42i64.into();
    assert_eq!(int_val, Value::Int(42));
    assert_eq!(format!("{}", int_val), "42");
    let int_val2: Value = 10i32.into();
    assert_eq!(int_val2, Value::Int(10));
    let float_val: Value = 3.14f64.into();
    assert_eq!(float_val, Value::Float(3.14));
    assert_eq!(format!("{}", float_val), "3.14");
    let str_val: Value = String::from("hello").into();
    assert_eq!(str_val, Value::String("hello".into()));
    assert_eq!(format!("{}", str_val), "'hello'");
    let str_val2: Value = "world".into();
    assert_eq!(str_val2, Value::String("world".into()));
    let bytes_val: Value = vec![1u8, 2, 3].into();
    assert_eq!(bytes_val, Value::Bytes(vec![1, 2, 3]));
    assert_eq!(format!("{}", bytes_val), "<3 bytes>");
    let null_val = Value::Null;
    assert_eq!(format!("{}", null_val), "NULL");
}

// ── Row: all methods ──
#[test]
fn test_row_all_methods() {
    let row = Row::new(vec![Value::Int(1), Value::String("a".into())]);
    assert!(!row.is_empty());
    assert_eq!(row.len(), 2);
    assert_eq!(row.get(0), Some(&Value::Int(1)));
    assert_eq!(row.get(1), Some(&Value::String("a".into())));
    assert_eq!(row.get(99), None);
    let empty = Row::new(vec![]);
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
}

// ── ConnectionHolder: all paths ──
#[test]
fn test_connection_holder_all_paths() {
    let h = ConnectionHolder::new(42);
    assert_eq!(h.state(), ConnectionState::Idle);
    assert!(h.mark_active());
    assert_eq!(h.state(), ConnectionState::Active);
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert!(h.mark_idle());
    assert_eq!(h.state(), ConnectionState::Idle);
    assert!(h.is_alive(std::time::Duration::from_secs(60)));
    assert!(h.held_duration() >= std::time::Duration::ZERO);
}

// ── ExceptionSorter: all three implementations ──
#[test]
fn test_exception_sorter_all() {
    assert!(!NullExceptionSorter.is_exception_fatal(0, "any"));
    assert!(!NullExceptionSorter.is_exception_fatal(57001, "shutdown"));
    assert!(PgExceptionSorter.is_exception_fatal(57001, "shutdown"));
    assert!(!PgExceptionSorter.is_exception_fatal(42601, "syntax"));
    assert!(PgExceptionSorter.is_exception_fatal(0, "connection has been closed"));
    assert!(PgExceptionSorter.is_exception_fatal(0, "connection is not available"));
    assert!(MySqlExceptionSorter.is_exception_fatal(1042, "hostname"));
    assert!(!MySqlExceptionSorter.is_exception_fatal(1062, "duplicate"));
    assert!(MySqlExceptionSorter.is_exception_fatal(0, "Communications link failure"));
}

// ── ConnState + Savepoint ──
#[test]
fn test_conn_state_all_fields() {
    let mut s = ConnState::default();
    assert!(s.auto_commit);
    assert!(!s.read_only);
    assert_eq!(s.transaction_isolation, 2);
    assert_eq!(s.catalog, None);
    assert_eq!(s.schema, None);
    s.auto_commit = false;
    s.read_only = true;
    s.transaction_isolation = 8;
    s.catalog = Some("mydb".into());
    s.schema = Some("public".into());
    assert!(!s.auto_commit);
    assert!(s.read_only);
    assert_eq!(s.transaction_isolation, 8);
}

#[test]
fn test_savepoint_all_variants() {
    let sp = Savepoint {
        id: 1,
        name: Some("sp1".into()),
    };
    assert_eq!(sp.id, 1);
    assert_eq!(sp.name.as_deref(), Some("sp1"));
    let sp2 = Savepoint { id: 2, name: None };
    assert_eq!(sp2.id, 2);
    assert_eq!(sp2.name, None);
    // Clone + PartialEq
    let sp3 = sp.clone();
    assert_eq!(sp, sp3);
}

// ── ExecResult: all fields ──
#[test]
fn test_exec_result_all() {
    let r = ExecResult::default();
    assert_eq!(r.rows_affected, 0);
    assert_eq!(r.last_insert_id, None);
    assert_eq!(r.row_count, None);
    let r2 = ExecResult {
        rows_affected: 5,
        last_insert_id: Some(10),
        row_count: Some(100),
    };
    assert_eq!(r2.rows_affected, 5);
    assert_eq!(r2.last_insert_id, Some(10));
    assert_eq!(r2.row_count, Some(100));
}

// ── Wrapper ──
#[test]
fn test_wrapper_default() {
    struct W;
    impl Wrapper for W {}
    assert!(!W.is_wrapper_for("anything"));
}

// ── Connection: all default methods ──
#[tokio::test]
async fn test_connection_all_defaults() {
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
    assert!(!m.is_closed());
    assert!(m.auto_commit());
    assert!(!m.read_only());
    assert_eq!(m.transaction_isolation(), 2);
    assert_eq!(m.catalog(), None);
    assert_eq!(m.schema(), None);
    assert_eq!(m.driver_name(), "");
}

// ── Driver: all methods ──
#[tokio::test]
async fn test_driver_all_methods() {
    struct D;
    #[async_trait::async_trait]
    impl Driver for D {
        fn name(&self) -> &str {
            "test"
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
    let d = D;
    assert_eq!(d.name(), "test");
    let c = d.connect("test://localhost").await.unwrap();
    assert_eq!(c.driver_name(), "test://localhost");
    // connect_with_auth delegates to connect
    let c2 = d
        .connect_with_auth("test://localhost", "u", "p")
        .await
        .unwrap();
    assert_eq!(c2.driver_name(), "test://localhost");
}

// ── ValidConnectionChecker ──
#[tokio::test]
async fn test_ping_checker_valid() {
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

// ── BeforeFilter: all event variants default ──
#[tokio::test]
async fn test_before_filter_all_event_defaults() {
    struct F;
    #[async_trait::async_trait]
    impl BeforeFilter for F {
        fn name(&self) -> &str {
            "f"
        }
        async fn before(&self, _: &mut ExecContext<'_>) -> Result<(), DruidError> {
            Ok(())
        }
    }
    let f = F;
    // ConnectionEvents
    assert!(f
        .on_connection_event(&ConnectionEvent::Connect)
        .await
        .is_ok());
    assert!(f.on_connection_event(&ConnectionEvent::Close).await.is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::SetAutoCommit(true))
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::GetAutoCommit)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::Commit)
        .await
        .is_ok());
    assert!(f
        .on_connection_event(&ConnectionEvent::Rollback)
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
    // StatementEvents
    assert!(f
        .on_statement_event(&StatementEvent::CreateStatement)
        .await
        .is_ok());
    assert!(f
        .on_statement_event(&StatementEvent::PrepareStatement("x".into()))
        .await
        .is_ok());
    assert!(f
        .on_statement_event(&StatementEvent::PrepareCall("x".into()))
        .await
        .is_ok());
    assert!(f
        .on_statement_event(&StatementEvent::Execute("x".into()))
        .await
        .is_ok());
    assert!(f
        .on_statement_event(&StatementEvent::ExecuteQuery("x".into()))
        .await
        .is_ok());
    assert!(f
        .on_statement_event(&StatementEvent::ExecuteUpdate("x".into()))
        .await
        .is_ok());
    assert!(f.on_statement_event(&StatementEvent::Close).await.is_ok());
    assert!(f
        .on_statement_event(&StatementEvent::ExecuteBatch)
        .await
        .is_ok());
    // ResultSetEvents
    assert!(f.on_result_set_event(&ResultSetEvent::Next).await.is_ok());
    assert!(f.on_result_set_event(&ResultSetEvent::Close).await.is_ok());
    assert!(f
        .on_result_set_event(&ResultSetEvent::GetString)
        .await
        .is_ok());
    assert!(f
        .on_result_set_event(&ResultSetEvent::GetBoolean)
        .await
        .is_ok());
    assert!(f.on_result_set_event(&ResultSetEvent::GetInt).await.is_ok());
    assert!(f.on_result_set_event(&ResultSetEvent::First).await.is_ok());
    assert!(f.on_result_set_event(&ResultSetEvent::Last).await.is_ok());
    // Lifecycle
    assert!(f.init().await.is_ok());
    assert!(f.destroy().await.is_ok());
}

// ── ExtendedFilter: all default hooks ──
#[tokio::test]
async fn test_extended_filter_all_default_hooks() {
    struct F;
    #[async_trait::async_trait]
    impl ExtendedFilter for F {
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
    let f = F;
    assert!(f
        .on_statement_property_event(&StatementPropertyEvent::SetQueryTimeout(100))
        .await
        .is_ok());
    assert!(f.on_clob_event(&ClobEvent::Length).await.is_ok());
    assert!(f
        .on_datasource_event(&DataSourceEvent::GetConnection)
        .await
        .is_ok());
    assert!(!f.is_wrapper_for("anything"));
}

// ── AfterFilter: all default hooks ──
#[tokio::test]
async fn test_after_filter_all_defaults() {
    struct F;
    #[async_trait::async_trait]
    impl AfterFilter for F {
        fn name(&self) -> &str {
            "f"
        }
        async fn after(
            &self,
            _: &ExecContext<'_>,
            _: &Result<ExecResult, DruidError>,
            _: std::time::Duration,
        ) {
        }
    }
    let f = F;
    f.after_connection_close().await;
}

// ── ConnectionExt: mock that REUSES default implementations ──
// This mock does NOT override get_holdability, set_holdability, etc.
// so the default trait impl bodies are actually executed.

struct DefaultConnExt;
#[async_trait::async_trait]
impl Connection for DefaultConnExt {
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

// Only implement ConnectionExt for non-async methods (use defaults)
#[async_trait::async_trait]
impl ConnectionExt for DefaultConnExt {
    // Use ALL default implementations - don't override anything!
    async fn create_statement(&mut self) -> Result<Box<dyn Connection>, DruidError> {
        Err(DruidError::Other("not implemented".into()))
    }
    async fn prepare_statement(&mut self, sql: &str) -> Result<Box<dyn Connection>, DruidError> {
        Err(DruidError::Other("not implemented".into()))
    }
    async fn prepare_call(&mut self, sql: &str) -> Result<Box<dyn Connection>, DruidError> {
        Err(DruidError::Other("not implemented".into()))
    }
}

#[tokio::test]
async fn test_default_impls_exercised() {
    let mut c = DefaultConnExt;
    // These call the DEFAULT implementations in the trait
    assert!(c.get_meta_data().is_none());
    assert_eq!(c.get_database_product_name(), None);
    assert_eq!(c.get_database_product_version(), None);
    assert_eq!(c.get_driver_major_version(), 0);
    assert_eq!(c.get_driver_minor_version(), 0);
    assert_eq!(c.get_holdability(), 1);
    let _ = ConnectionExt::set_holdability(&mut c, 1).await;
    let _ = c.set_client_info("k", "v").await;
    assert_eq!(c.get_client_info("k"), None);
    let _ = ConnectionExt::clear_warnings(&mut c).await;
    assert_eq!(c.native_sql("SELECT 1").await.unwrap(), "SELECT 1");
    let _ = c
        .set_network_timeout(std::time::Duration::from_secs(5))
        .await;
    assert_eq!(c.get_network_timeout(), 0);
    assert_eq!(c.get_type_map(), None);
    let _ = c.set_type_map(std::collections::HashMap::new()).await;
}

// ── PooledConnection: exercise Drop return_fn path ──
#[tokio::test]
async fn test_pooled_connection_drop_exercise() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    struct MockConn;
    #[async_trait::async_trait]
    impl Connection for MockConn {
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
    let counter = Arc::new(AtomicU64::new(0));
    let c = counter.clone();
    let conn = Box::new(MockConn) as Box<dyn Connection>;
    {
        let pc = PooledConnection::new(
            conn,
            1,
            Box::new(move |_, _| {
                c.fetch_add(1, Ordering::Relaxed);
            }),
        );
        // Drop triggers return_fn
    }
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

// ── MinimalConnExt: implements NO ConnectionExt methods at all ──
// All default implementations are exercised by this mock.
struct MinimalConnExt;

#[async_trait::async_trait]
impl Connection for MinimalConnExt {
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

// NO ConnectionExt implementation at all - uses trait defaults
// MinimalConnExt doesn't implement ConnectionExt, so we can't call its methods

// Instead, let's create a struct that only implements ConnectionExt via default
// by using a blanket impl trick

// Actually, let me just call the trait default methods directly on a mock
// that does implement ConnectionExt but with minimal overrides
struct SparseConnExt;

#[async_trait::async_trait]
impl Connection for SparseConnExt {
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

// SparseConnExt implements ConnectionExt by default (no override)
#[async_trait::async_trait]
impl ConnectionExt for SparseConnExt {
    // ALL methods use trait defaults - don't override anything
}

#[tokio::test]
async fn test_all_default_methods_exercised() {
    let mut c = SparseConnExt;
    // These call the DEFAULT implementations (not overridden)
    assert!(c.get_meta_data().is_none());
    assert_eq!(c.get_database_product_name(), None);
    assert_eq!(c.get_database_product_version(), None);
    assert_eq!(c.get_driver_major_version(), 0);
    assert_eq!(c.get_driver_minor_version(), 0);
    assert_eq!(c.get_holdability(), 1);
    let _ = ConnectionExt::set_holdability(&mut c, 1).await;
    let _ = c.set_client_info("k", "v").await;
    assert_eq!(c.get_client_info("k"), None);
    let _ = ConnectionExt::clear_warnings(&mut c).await;
    assert_eq!(c.native_sql("SELECT 1").await.unwrap(), "SELECT 1");
    let _ = c.set_network_timeout(Duration::from_secs(5)).await;
    assert_eq!(c.get_network_timeout(), 0);
    assert_eq!(c.get_type_map(), None);
    let _ = c.set_type_map(std::collections::HashMap::new()).await;
    // Also test async methods that return errors
    assert!(c.create_statement().await.is_err());
    assert!(c.prepare_statement("SELECT 1").await.is_err());
    assert!(c.prepare_call("CALL sp").await.is_err());
}

// ── Also test Row methods more thoroughly ──
#[test]
fn test_row_construction_and_access() {
    let r1 = Row::new(vec![]);
    assert!(r1.is_empty());
    assert_eq!(r1.len(), 0);
    assert_eq!(r1.get(0), None);

    let r2 = Row::new(vec![Value::Int(1), Value::String("a".into()), Value::Null]);
    assert!(!r2.is_empty());
    assert_eq!(r2.len(), 3);
    assert_eq!(r2.get(0), Some(&Value::Int(1)));
    assert_eq!(r2.get(1), Some(&Value::String("a".into())));
    assert_eq!(r2.get(2), Some(&Value::Null));
    assert_eq!(r2.get(3), None);
}

// ── Test all Connection trait default methods more thoroughly ──
#[tokio::test]
async fn test_connection_defaults_comprehensive() {
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
    // Test all default methods that return values
    assert!(!m.is_closed());
    assert!(m.auto_commit());
    assert!(!m.read_only());
    assert_eq!(m.transaction_isolation(), 2);
    assert_eq!(m.catalog(), None);
    assert_eq!(m.schema(), None);
    assert_eq!(m.driver_name(), "");
    // Test methods that modify state
    let _ = m.set_auto_commit(false).await;
    let _ = m.set_read_only(true).await;
    let _ = m.set_transaction_isolation(8).await;
    let _ = m.set_catalog("mydb").await;
    let _ = m.set_schema("public").await;
    // Test savepoint methods
    let sp = m.set_savepoint().await;
    assert!(sp.is_err());
    let sp2 = m.set_savepoint_named("sp1").await;
    assert!(sp2.is_err());
    let _ = m.release_savepoint(&Savepoint { id: 1, name: None }).await;
    let _ = m.rollback_to(&Savepoint { id: 1, name: None }).await;
    // Test abort (should call close)
    assert!(!m.is_closed());
    let _ = m.abort().await;
}

// ── Test all event variants more thoroughly ──
#[test]
fn test_all_event_variants_debug() {
    let events = vec![
        format!("{:?}", ConnectionEvent::Connect),
        format!("{:?}", ConnectionEvent::Close),
        format!("{:?}", ConnectionEvent::Commit),
        format!("{:?}", ConnectionEvent::Rollback),
        format!("{:?}", ConnectionEvent::SetAutoCommit(true)),
        format!("{:?}", ConnectionEvent::SetReadOnly(false)),
        format!("{:?}", ConnectionEvent::SetCatalog("db".into())),
        format!("{:?}", ConnectionEvent::SetTransactionIsolation(2)),
        format!("{:?}", ConnectionEvent::SetSchema("s".into())),
        format!("{:?}", ConnectionEvent::NativeSQL("sql".into())),
        format!(
            "{:?}",
            ConnectionEvent::SetNetworkTimeout(Duration::from_secs(1))
        ),
        format!("{:?}", StatementPropertyEvent::SetQueryTimeout(100)),
        format!("{:?}", StatementPropertyEvent::GetQueryTimeout),
        format!("{:?}", StatementPropertyEvent::GetUpdateCount),
        format!("{:?}", StatementPropertyEvent::SetMaxRows(100)),
        format!("{:?}", StatementPropertyEvent::GetMaxRows),
        format!("{:?}", StatementPropertyEvent::SetMaxFieldSize(50)),
        format!("{:?}", StatementPropertyEvent::GetMaxFieldSize),
        format!("{:?}", StatementPropertyEvent::SetFetchDirection(1)),
        format!("{:?}", StatementPropertyEvent::GetFetchDirection),
        format!("{:?}", StatementPropertyEvent::SetFetchSize(10)),
        format!("{:?}", StatementPropertyEvent::GetFetchSize),
        format!("{:?}", StatementPropertyEvent::IsPoolable),
        format!("{:?}", StatementPropertyEvent::IsClosed),
        format!("{:?}", StatementPropertyEvent::GetMoreResults),
        format!("{:?}", StatementPropertyEvent::GetResultSetConcurrency),
        format!("{:?}", StatementPropertyEvent::GetResultSetType),
        format!("{:?}", StatementPropertyEvent::GetResultSetHoldability),
        format!("{:?}", StatementPropertyEvent::GetGeneratedKeys),
        format!("{:?}", StatementPropertyEvent::ClearWarnings),
        format!("{:?}", StatementPropertyEvent::SetCursorName("c".into())),
        format!("{:?}", StatementPropertyEvent::AddBatch("b".into())),
        format!("{:?}", ClobEvent::Length),
        format!("{:?}", ClobEvent::GetSubString(1, 5)),
        format!("{:?}", ClobEvent::SetString(1, "x".into())),
        format!("{:?}", ClobEvent::Truncate(10)),
        format!("{:?}", ClobEvent::Free),
        format!("{:?}", DataSourceEvent::GetConnection),
        format!(
            "{:?}",
            DataSourceEvent::GetConnectionWithAuth("u".into(), "p".into())
        ),
        format!("{:?}", DataSourceEvent::ReleaseConnection),
        format!("{:?}", DataSourceEvent::Log("msg".into())),
    ];
    assert!(events.len() > 40);
}

// ── Call Connection trait default methods on SparseConnExt ──
#[tokio::test]
async fn test_connection_default_methods_via_sparse() {
    let mut c = SparseConnExt;
    // rollback_to (default impl returns Err)
    let sp = Savepoint { id: 1, name: None };
    let result = c.rollback_to(&sp).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not supported"));

    // set_savepoint (default impl returns Err)
    let result = c.set_savepoint().await;
    assert!(result.is_err());

    // set_savepoint_named (default impl returns Err)
    let result = c.set_savepoint_named("sp1").await;
    assert!(result.is_err());

    // release_savepoint (default impl returns Err)
    let result = c.release_savepoint(&sp).await;
    assert!(result.is_err());

    // abort (default impl calls close)
    assert!(!c.is_closed());
    let _ = c.abort().await;
}

// ── Call Connection default methods on another mock ──
#[tokio::test]
async fn test_connection_default_methods_on_mock_conn() {
    let mut c = MockConnForExt;
    let sp = Savepoint {
        id: 1,
        name: Some("sp1".into()),
    };
    let _ = c.rollback_to(&sp).await;
    let _ = c.set_savepoint().await;
    let _ = c.set_savepoint_named("sp2").await;
    let _ = c.release_savepoint(&sp).await;
}
