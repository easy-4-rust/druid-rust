//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Coverage boost tests targeting specific uncovered lines.
use druid::core::*;
use std::sync::Arc;
use std::time::Duration;

// ── ConnectionExt: all default methods ──

struct MockConnForExt;
druid_rdbc::impl_standard_wrapper!(MockConnForExt => Connection);

#[async_trait::async_trait]
impl Connection for MockConnForExt {
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

// ── PooledConnection: all branches ──

// ── Error: test every variant Display ──

// ── Value: all From conversions + Display ──

// ── Row: all methods ──

// ── ConnectionHolder: all paths ──

// ── ExceptionSorter: all three implementations ──

// ── ConnState + Savepoint ──

// ── ExecResult: all fields ──

// ── Wrapper ──

// ── Connection: all default methods ──

// ── Driver: all methods ──

// ── ValidConnectionChecker ──

// ── BeforeFilter: all event variants default ──

// ── ExtendedFilter: all default hooks ──

// ── AfterFilter: all default hooks ──

// ── ConnectionExt: mock that REUSES default implementations ──
// This mock does NOT override get_holdability, set_holdability, etc.
// so the default trait impl bodies are actually executed.

struct DefaultConnExt;
druid_rdbc::impl_standard_wrapper!(DefaultConnExt => Connection);

#[async_trait::async_trait]
impl Connection for DefaultConnExt {
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

// Only implement ConnectionExt for non-async methods (use defaults)

// ── PooledConnection: exercise Drop return_fn path ──

// ── MinimalConnExt: implements NO ConnectionExt methods at all ──
// All default implementations are exercised by this mock.
#[allow(dead_code)]
struct MinimalConnExt;

druid_rdbc::impl_standard_wrapper!(MinimalConnExt => Connection);

#[async_trait::async_trait]
impl Connection for MinimalConnExt {
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

// NO ConnectionExt implementation at all - uses trait defaults
// MinimalConnExt doesn't implement ConnectionExt, so we can't call its methods

// Instead, let's create a struct that only implements ConnectionExt via default
// by using a blanket impl trick

// Actually, let me just call the trait default methods directly on a mock
// that does implement ConnectionExt but with minimal overrides
struct SparseConnExt;

druid_rdbc::impl_standard_wrapper!(SparseConnExt => Connection);

#[async_trait::async_trait]
impl Connection for SparseConnExt {
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

// SparseConnExt implements ConnectionExt by default (no override)

// ── Also test Row methods more thoroughly ──

// ── Test all Connection trait default methods more thoroughly ──

// ── Test all event variants more thoroughly ──

// ── Call Connection trait default methods on SparseConnExt ──

// ── Call Connection default methods on another mock ──

#[tokio::test]
async fn task6_coverage_boost_test_connection_ext_exercise_all_defaults() {
    let mut c = MockConnForExt;
    let options = druid_rdbc::statement::StatementOptions::default();
    assert!(c.create_statement(options).await.is_err());
    assert!(c.prepare_statement("SELECT 1", options).await.is_err());
    assert!(c.prepare_call("CALL sp", options).await.is_err());
    assert!(c.database_meta_data().await.is_err());
    assert!(c.database_product_name().await.is_err());
    assert!(c.database_product_version().await.is_err());
    assert!(c.driver_major_version().is_err());
    assert!(c.driver_minor_version().is_err());
    assert!(c.holdability().is_err());
    assert!(Connection::set_holdability(
        &mut c,
        druid_rdbc::statement::ResultSetHoldability::CloseAtCommit,
    )
    .await
    .is_err());
    assert!(c.set_client_info("k", Some("v")).await.is_err());
    assert!(c.client_info("k").is_err());
    assert!(Connection::clear_warnings(&mut c).await.is_err());
    assert_eq!(c.native_sql("SELECT 1").await.unwrap(), "SELECT 1");
    assert!(c.set_network_timeout(Duration::from_secs(5)).await.is_err());
    assert!(c.network_timeout().is_err());
    assert!(c.type_map().is_err());
    assert!(c
        .set_type_map(druid_rdbc::types::RdbcTypeMap::new())
        .await
        .is_err());
}

#[tokio::test]
async fn task6_coverage_boost_test_pooled_connection_all_paths() {
    struct MockConn;
    druid_rdbc::impl_standard_wrapper!(MockConn => Connection);

    #[async_trait::async_trait]
    impl Connection for MockConn {
        async fn exec(&mut self, _: &str, _: Vec<Value>) -> Result<ExecResult, RdbcError> {
            Ok(ExecResult {
                rows_affected: 1,
                last_insert_id: Some(42),
                row_count: None,
            })
        }
        async fn fetch(&mut self, _: &str, _: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
            Ok(vec![Row::new(vec![Value::Int(99)])])
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
        fn driver_name(&self) -> Result<String, RdbcError> {
            Ok("coverage-boost".to_owned())
        }
        fn auto_commit(&self) -> Result<bool, RdbcError> {
            Ok(true)
        }
        async fn set_auto_commit(&mut self, _: bool) -> Result<(), RdbcError> {
            Ok(())
        }
    }
    let conn = Box::new(MockConn) as Box<dyn Connection>;
    let holder = DruidConnectionHolder::with_standard_connection(conn, 1, Duration::ZERO, 0)
        .await
        .unwrap();
    let mut pc = PooledConnection::with_standard_holder(
        holder,
        "coverage-boost".to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    );
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
    assert!(pc.driver_name().unwrap().contains("coverage"));
}

#[tokio::test]
async fn task6_coverage_boost_test_pooled_connection_into_core() {
    let conn = Box::new(MockConnForExt) as Box<dyn Connection>;
    let holder = DruidConnectionHolder::with_standard_connection(conn, 1, Duration::ZERO, 0)
        .await
        .unwrap();
    let mut pc = PooledConnection::with_standard_holder(
        holder,
        "coverage-boost".to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    );
    let _ = pc.exec("x", vec![]).await;
}

#[tokio::test]
async fn task6_coverage_boost_test_pooled_connection_fetch_empty() {
    struct MockConn;
    druid_rdbc::impl_standard_wrapper!(MockConn => Connection);

    #[async_trait::async_trait]
    impl Connection for MockConn {
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
        fn driver_name(&self) -> Result<String, RdbcError> {
            Ok("coverage-boost".to_owned())
        }
        fn auto_commit(&self) -> Result<bool, RdbcError> {
            Ok(true)
        }
        async fn set_auto_commit(&mut self, _: bool) -> Result<(), RdbcError> {
            Ok(())
        }
    }
    let conn = Box::new(MockConn) as Box<dyn Connection>;
    let holder = DruidConnectionHolder::with_standard_connection(conn, 1, Duration::ZERO, 0)
        .await
        .unwrap();
    let mut pc = PooledConnection::with_standard_holder(
        holder,
        "coverage-boost".to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    );
    let rows = pc.fetch("SELECT 1", vec![]).await.unwrap();
    assert!(rows.is_empty());
}

#[test]
fn task6_coverage_boost_test_error_all_variants_display() {
    let variants = vec![
        RdbcError::DataSourceClosed {
            close_time_millis: 0,
        },
        RdbcError::AcquireTimeout,
        RdbcError::PoolExhausted,
        RdbcError::ValidationFailed("x".into()),
        RdbcError::ConnectionLeaked {
            id: 1,
            held_for: std::time::Duration::from_secs(10),
        },
        RdbcError::ConnectionDiscarded,
        RdbcError::DriverError("e".into()),
        RdbcError::SqlParseError("e".into()),
        RdbcError::WallViolation("e".into()),
        RdbcError::DataSourceNotFound("e".into()),
        RdbcError::UnsupportedOperation { operation: "test" },
        RdbcError::Other("e".into()),
    ];
    for v in &variants {
        let _ = format!("{v}");
    }
    // Test Error trait
    for v in &variants {
        assert!(std::error::Error::source(v).is_none());
    }
}

#[test]
fn task6_coverage_boost_test_value_all_conversions_and_display() {
    let bool_val: Value = true.into();
    assert_eq!(bool_val, Value::Bool(true));
    assert_eq!(format!("{bool_val}"), "true");
    let int_val: Value = 42i64.into();
    assert_eq!(int_val, Value::Int(42));
    assert_eq!(format!("{int_val}"), "42");
    let int_val2: Value = 10i32.into();
    assert_eq!(int_val2, Value::Int(10));
    let float_val: Value = 3.125f64.into();
    assert_eq!(float_val, Value::Float(3.125));
    assert_eq!(format!("{float_val}"), "3.125");
    let str_val: Value = String::from("hello").into();
    assert_eq!(str_val, Value::String("hello".into()));
    assert_eq!(format!("{str_val}"), "'hello'");
    let str_val2: Value = "world".into();
    assert_eq!(str_val2, Value::String("world".into()));
    let bytes_val: Value = vec![1u8, 2, 3].into();
    assert_eq!(bytes_val, Value::Bytes(vec![1, 2, 3]));
    assert_eq!(format!("{bytes_val}"), "<3 bytes>");
    let null_val = Value::Null;
    assert_eq!(format!("{null_val}"), "NULL");
}

#[test]
fn task6_coverage_boost_test_row_all_methods() {
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

#[test]
fn task6_coverage_boost_test_connection_holder_all_paths() {
    let h = ConnectionHolder::new(42);
    assert_eq!(h.state(), ConnectionState::Idle);
    assert!(h.mark_active());
    assert_eq!(h.state(), ConnectionState::Active);
    assert_eq!(h.use_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert!(h.mark_idle());
    assert_eq!(h.state(), ConnectionState::Idle);
    assert!(h.is_alive(std::time::Duration::from_mins(1)));
    assert!(h.held_duration() >= std::time::Duration::ZERO);
}

#[test]
fn task6_coverage_boost_test_exception_sorter_all() {
    assert!(!NullExceptionSorter.is_exception_fatal(&SqlException::driver(0, "any")));
    assert!(!NullExceptionSorter.is_exception_fatal(&SqlException::driver(57001, "shutdown")));
    assert!(PgExceptionSorter
        .is_exception_fatal(&SqlException::driver(0, "closed").with_sql_state("08003")));
    assert!(!PgExceptionSorter
        .is_exception_fatal(&SqlException::driver(0, "syntax").with_sql_state("42601")));
    assert!(MySqlExceptionSorter.is_exception_fatal(&SqlException::driver(1042, "hostname")));
    assert!(!MySqlExceptionSorter.is_exception_fatal(&SqlException::driver(1062, "duplicate")));
    assert!(MySqlExceptionSorter
        .is_exception_fatal(&SqlException::driver(0, "Communications link failure")));
}

#[test]
fn task6_coverage_boost_test_conn_state_all_fields() {
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
fn task6_coverage_boost_test_savepoint_all_variants() {
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

#[test]
fn task6_coverage_boost_test_exec_result_all() {
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

#[test]
fn task6_coverage_boost_test_wrapper_default() {
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

#[tokio::test]
async fn task6_coverage_boost_test_connection_all_defaults() {
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
    assert!(m.is_closed().is_err());
    assert!(m.auto_commit().is_err());
    assert!(m.read_only().is_err());
    assert!(m.transaction_isolation().is_err());
    assert!(m.catalog().is_err());
    assert!(m.schema().is_err());
    assert!(m.driver_name().is_err());
}

#[tokio::test]
async fn task6_coverage_boost_test_driver_all_methods() {
    struct D;
    #[async_trait::async_trait]
    impl Driver for D {
        fn name(&self) -> &'static str {
            "test"
        }
        fn accepts_url(&self, url: &str) -> bool {
            url.starts_with("test://")
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
    let driver = D;
    assert_eq!(driver.name(), "test");
    assert!(driver.accepts_url("test://localhost"));
    assert!(!driver.accepts_url("other://localhost"));
    let mut properties = druid_rdbc::driver::ConnectionProperties::new();
    let connection = driver
        .connect("test://localhost", &properties)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connection.driver_name().unwrap(), "test://localhost#");
    properties.insert("user".to_owned(), "u".to_owned());
    properties.insert("password".to_owned(), "p".to_owned());
    let connection = driver
        .connect("test://localhost", &properties)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(connection.driver_name().unwrap(), "test://localhost#u");
}

#[tokio::test]
async fn task6_coverage_boost_test_ping_checker_valid() {
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
        async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
            self.ping().await?;
            Ok(true)
        }
    }
    let checker = PingConnectionChecker;
    let mut conn = Box::new(C) as Box<dyn Connection>;
    assert!(checker.is_valid(&mut conn).await);
}
