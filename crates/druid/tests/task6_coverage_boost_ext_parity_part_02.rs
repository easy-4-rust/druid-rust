//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Comprehensive coverage boost tests for druid-core.
//!
//! Targets: config.rs (47 uncovered), filter.rs (20 uncovered),
//! `pooled_connection.rs` (31 uncovered), connection.rs (52 uncovered),
//! `connection_holder.rs` (29 uncovered), error.rs (16 uncovered),
//! value.rs (16 uncovered), `exception_sorter.rs` (11 uncovered).

use druid::core::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// ══════════════════════════════════════════════════════════════════
// Mock Connection + ConnectionExt + ConnectionFactory
// ══════════════════════════════════════════════════════════════════

struct MockConnForPool;

druid_rdbc::impl_standard_wrapper!(MockConnForPool => Connection);

#[async_trait::async_trait]
impl Connection for MockConnForPool {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: Some(42),
            row_count: None,
        })
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Ok(vec![Row::new(vec![
            Value::Int(1),
            Value::String("test".into()),
        ])])
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
        Ok("mock-pool".to_owned())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(true)
    }
    async fn set_auto_commit(&mut self, _auto_commit: bool) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        self.ping().await?;
        Ok(true)
    }
}

// ══════════════════════════════════════════════════════════════════
// 1. config.rs: PoolConfig default + all builder methods (47 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 2. filter.rs: ExtendedFilter default methods (20 lines)
// ══════════════════════════════════════════════════════════════════

struct MockExtendedFilter;

#[async_trait::async_trait]
impl ExtendedFilter for MockExtendedFilter {}

// ══════════════════════════════════════════════════════════════════
// 3. pooled_connection.rs: exec/fetch/begin/commit/rollback/ping (31 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 4. connection_holder.rs: state machine (29 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 5. error.rs: Display for all variants (16 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 6. value.rs: Display + From conversions (16 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 7. exception_sorter.rs (11 lines)
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 8. connection.rs: All default method bodies (52 lines)
// ══════════════════════════════════════════════════════════════════

struct MockConnForDefaults;

druid_rdbc::impl_standard_wrapper!(MockConnForDefaults => Connection);

#[async_trait::async_trait]
impl Connection for MockConnForDefaults {
    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        Ok(true)
    }
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

// ══════════════════════════════════════════════════════════════════
// 9. connection_factory.rs
// ══════════════════════════════════════════════════════════════════

struct MockConnectionFactory;

#[async_trait::async_trait]
impl ConnectionFactory for MockConnectionFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(MockConnForDefaults))
    }
    async fn validate(&self, _conn: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        Ok(())
    }
}

// ══════════════════════════════════════════════════════════════════
// 10. MetaData struct
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 11. StatementType enum
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 12. ConnState
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 13. ValidConnectionChecker
// ══════════════════════════════════════════════════════════════════

#[test]
fn task6_coverage_boost_ext_test_meta_data_clone() {
    let md = MetaData {
        database_product_name: "PostgreSQL".into(),
        database_product_version: "14.0".into(),
        driver_name: "postgres".into(),
        driver_version: "0.1.0".into(),
        driver_major_version: 0,
        driver_minor_version: 1,
    };
    let md2 = md.clone();
    assert_eq!(md.database_product_name, md2.database_product_name);
}

#[test]
fn task6_coverage_boost_ext_test_statement_type_variants() {
    let st = StatementType::Statement;
    let debug = format!("{st:?}");
    assert!(debug.contains("Statement"));

    let st = StatementType::PreparedStatement("SELECT 1".into());
    let debug = format!("{st:?}");
    assert!(debug.contains("PreparedStatement"));

    let st = StatementType::CallableStatement("CALL sp()".into());
    let debug = format!("{st:?}");
    assert!(debug.contains("CallableStatement"));
}

#[test]
fn task6_coverage_boost_ext_test_conn_state_clone_and_debug() {
    let s = ConnState {
        auto_commit: false,
        read_only: true,
        transaction_isolation: 8,
        catalog: Some("mydb".into()),
        schema: Some("public".into()),
    };

    let s2 = s.clone();
    assert_eq!(s.auto_commit, s2.auto_commit);
    assert_eq!(s.read_only, s2.read_only);
    assert_eq!(s.transaction_isolation, s2.transaction_isolation);
    assert_eq!(s.catalog, s2.catalog);
    assert_eq!(s.schema, s2.schema);

    let debug = format!("{s:?}");
    assert!(debug.contains("ConnState"));
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_valid_connection_checker_ping() {
    let checker = PingConnectionChecker;
    let mut conn: Box<dyn Connection> = Box::new(MockConnForPool);
    assert!(checker.is_valid(&mut conn).await);
}

#[tokio::test]
async fn task6_coverage_boost_ext_test_valid_connection_checker_close_default() {
    let factory = MockConnectionFactory;
    let mut conn = factory.create().await.unwrap();
    factory.close(&mut conn).await.unwrap();
}
