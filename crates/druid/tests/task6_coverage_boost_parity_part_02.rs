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
async fn task6_coverage_boost_test_before_filter_all_event_defaults() {
    struct F;
    #[async_trait::async_trait]
    impl BeforeFilter for F {
        fn name(&self) -> &'static str {
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

#[tokio::test]
async fn task6_coverage_boost_test_extended_filter_all_default_hooks() {
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

#[tokio::test]
async fn task6_coverage_boost_test_after_filter_all_defaults() {
    struct F;
    #[async_trait::async_trait]
    impl AfterFilter for F {
        fn name(&self) -> &'static str {
            "f"
        }
        async fn after(
            &self,
            _: &ExecContext<'_>,
            _: &Result<ExecResult, DruidError>,
            _: std::time::Duration,
        ) -> Result<(), DruidError> {
            Ok(())
        }
    }
    let f = F;
    f.after_connection_event(&ConnectionEvent::Rollback, Duration::ZERO)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_coverage_boost_test_default_impls_exercised() {
    let mut c = DefaultConnExt;
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
async fn task6_coverage_boost_test_pooled_connection_drop_exercise() {
    use std::sync::atomic::{AtomicU64, Ordering};

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
    }
    let counter = Arc::new(AtomicU64::new(0));
    let returned = counter.clone();
    let holder =
        DruidConnectionHolder::with_standard_connection(Box::new(MockConn), 1, Duration::ZERO, 0)
            .await
            .unwrap();
    {
        let _connection = DruidPooledConnection::with_standard_holder(
            holder,
            "coverage-boost".to_owned(),
            None,
            false,
            false,
            Box::new(move |_, _| {
                returned.fetch_add(1, Ordering::Relaxed);
                true
            }),
        );
    }
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn task6_coverage_boost_test_all_default_methods_exercised() {
    let mut c = SparseConnExt;
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

#[test]
fn task6_coverage_boost_test_row_construction_and_access() {
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

#[tokio::test]
async fn task6_coverage_boost_test_connection_defaults_comprehensive() {
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
    assert!(m.is_closed().is_err());
    assert!(m.auto_commit().is_err());
    assert!(m.read_only().is_err());
    assert!(m.transaction_isolation().is_err());
    assert!(m.catalog().is_err());
    assert!(m.schema().is_err());
    assert!(m.driver_name().is_err());
    assert!(m.set_auto_commit(false).await.is_err());
    assert!(m.set_read_only(true).await.is_err());
    assert!(m
        .set_transaction_isolation(druid_rdbc::connection::TransactionIsolation::SERIALIZABLE)
        .await
        .is_err());
    assert!(m.set_catalog(Some("mydb")).await.is_err());
    assert!(m.set_schema(Some("public")).await.is_err());
    let savepoint = m.set_savepoint().await;
    assert!(savepoint.is_err());
    let named = m.set_savepoint_named("sp1").await;
    assert!(named.is_err());
    let savepoint = Savepoint { id: 1, name: None };
    assert!(m.release_savepoint(&savepoint).await.is_err());
    assert!(m.rollback_to(&savepoint).await.is_err());
    assert!(m.abort().await.is_ok());
}

#[test]
fn task6_coverage_boost_test_all_event_variants_debug() {
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

#[tokio::test]
async fn task6_coverage_boost_test_connection_default_methods_via_sparse() {
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
    assert!(c.is_closed().is_err());
    assert!(c.abort().await.is_ok());
}

#[tokio::test]
async fn task6_coverage_boost_test_connection_default_methods_on_mock_conn() {
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
