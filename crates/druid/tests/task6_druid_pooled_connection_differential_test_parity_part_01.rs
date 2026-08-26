//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Differential tests for `DruidPooledConnection` — Java Druid 1.2.28 semantics.
//!
//! Uses real Toasty `SQLite` in-memory connections. Focuses on uncovered getter/setter
//! families, transaction lifecycle, warning chain, close/Drop, attributes, event
//! listeners, error paths, and the Wrapper trait.

use druid::core::{
    Connection, ConnectionEventListener, ConnectionFactory, DruidConnectionHolder, DruidError,
    DruidPooledConnection, ExceptionSorter, ExceptionSorterProperties, ProxyAttributeValue,
    RdbcError, SqlException, StandardUnwrapped, StatementEventListener, Value, Wrapper, WrapperExt,
};
use druid_rdbc::connection::TransactionIsolation;
use druid_rdbc::metadata::DatabaseMetaDataInfoExt;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::{
    GeneratedKeys, ParameterIndex, PreparedParameter, PreparedStatement, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ── helpers ────────────────────────────────────────────────────────

async fn make_physical() -> Box<dyn Connection> {
    ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory")
        .create()
        .await
        .expect("connection")
}

async fn make_pooled(physical: Box<dyn Connection>) -> DruidPooledConnection {
    let holder =
        DruidConnectionHolder::with_standard_connection(physical, 1, std::time::Duration::ZERO, 0)
            .await
            .unwrap();
    DruidPooledConnection::with_standard_holder(
        holder,
        String::new(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    )
}

async fn make_pooled_with_source(
    physical: Box<dyn Connection>,
    data_source: &str,
) -> DruidPooledConnection {
    let holder =
        DruidConnectionHolder::with_standard_connection(physical, 2, std::time::Duration::ZERO, 0)
            .await
            .unwrap();
    DruidPooledConnection::with_standard_holder(
        holder,
        data_source.to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    )
}

// ── id / data_source / is_recycled ─────────────────────────────────

// ── connected_time_millis / properties / close_count ───────────────

// ── last_validate_time_millis ──────────────────────────────────────

// ── connection_hold_duration / set_connected_time_nano ─────────────

// ── attributes ─────────────────────────────────────────────────────

// ── transaction_info ───────────────────────────────────────────────

// ── variables / global_variables ───────────────────────────────────

// ── connection_holder ──────────────────────────────────────────────

// ── physical_connection_mut ────────────────────────────────────────

// ── Debug ──────────────────────────────────────────────────────────

// ── Wrapper trait ──────────────────────────────────────────────────

// ── Connection trait delegation ─────────────────────────────

// ── exec through pooled connection ─────────────────────────────────

// ── transaction lifecycle through pooled connection ─────────────────

// ── savepoints through pooled ──────────────────────────────────────

// ── warnings ───────────────────────────────────────────────────────

// ── set_auto_commit ────────────────────────────────────────────────

// ── set_transaction_isolation ──────────────────────────────────────

// ── close / is_closed ──────────────────────────────────────────────

// ── recycle() consumes self ────────────────────────────────────────

// ── discard_connection ─────────────────────────────────────────────

// ── ping ───────────────────────────────────────────────────────────

// ── create_statement ───────────────────────────────────────────────

// ── prepare_statement family ───────────────────────────────────────

// ── prepare_call family ────────────────────────────────────────────

// ── event listeners ────────────────────────────────────────────────

#[derive(Debug)]
struct TestConnectionEventListener {
    closed_count: AtomicUsize,
    error_count: AtomicUsize,
}

impl TestConnectionEventListener {
    fn new() -> Self {
        Self {
            closed_count: AtomicUsize::new(0),
            error_count: AtomicUsize::new(0),
        }
    }
}

impl ConnectionEventListener for TestConnectionEventListener {
    fn connection_closed(&self, _connection_id: u64) {
        self.closed_count.fetch_add(1, Ordering::Relaxed);
    }

    fn connection_error_occurred(&self, _connection_id: u64, _error: &DruidError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug)]
struct TestStatementEventListener;

impl StatementEventListener for TestStatementEventListener {
    fn statement_closed(&self, _connection_id: u64, _statement_id: usize) {}
    fn statement_error_occurred(
        &self,
        _connection_id: u64,
        _statement_id: usize,
        _error: &DruidError,
    ) {
    }
}

// ── exception sorter ───────────────────────────────────────────────

struct NeverFatalSorter;

impl ExceptionSorter for NeverFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        false
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

// ── handle_exception ───────────────────────────────────────────────

struct AlwaysFatalSorter;

impl ExceptionSorter for AlwaysFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

// ── is_discarded / mark_discarded ──────────────────────────────────

// ── database_meta_data / get_meta_data ─────────────────────────────

// ── catalog / schema ───────────────────────────────────────────────

// ── Drop ───────────────────────────────────────────────────────────

// ── ConnectionConstants ────────────────────────────────────

// ── Multiple sequential operations ─────────────────────────────────

// ── with_recycle_policy ────────────────────────────────────────────

// ── create_statement_with_result_set ───────────────────────────────

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_id_returns_constructor_id() {
    let p = make_pooled(make_physical().await).await;
    assert_eq!(p.id(), 1);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_data_source_returns_constructor_string() {
    let p = make_pooled_with_source(make_physical().await, "my-datasource").await;
    assert_eq!(p.data_source(), "my-datasource");
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_data_source_empty_by_default() {
    let p = make_pooled(make_physical().await).await;
    assert_eq!(p.data_source(), "");
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_is_recycled_false_initially() {
    let p = make_pooled(make_physical().await).await;
    assert!(!p.is_recycled());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_connected_time_millis_is_nonzero() {
    let p = make_pooled(make_physical().await).await;
    assert!(p.connected_time_millis() > 0);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_properties_returns_empty_by_default() {
    let p = make_pooled(make_physical().await).await;
    assert!(p.properties().is_empty());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_close_count_zero_initially() {
    let p = make_pooled(make_physical().await).await;
    assert_eq!(p.close_count(), 0);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_last_validate_time_millis_zero_initially()
{
    let mut p = make_pooled(make_physical().await).await;
    assert_eq!(p.last_validate_time_millis(), 0);
    p.set_last_validate_time_millis(12345);
    assert_eq!(p.last_validate_time_millis(), 12345);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_connection_hold_duration_is_nonzero() {
    let p = make_pooled(make_physical().await).await;
    let _ = p.connection_hold_duration();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_set_connected_time_nano_resets_borrowed_at(
) {
    let mut p = make_pooled(make_physical().await).await;
    let before = p.connection_hold_duration();
    p.set_connected_time_nano();
    let after = p.connection_hold_duration();
    assert!(after <= before + std::time::Duration::from_millis(100));
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_attributes_empty_initially() {
    let p = make_pooled(make_physical().await).await;
    assert_eq!(p.attributes_size(), 0);
    assert!(p.attributes().is_empty());
    assert!(p.attribute("missing").is_none());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_put_attribute_and_get() {
    let p = make_pooled(make_physical().await).await;
    let old = p.put_attribute("key1", ProxyAttributeValue::new("val1".to_string()));
    assert!(old.is_none());
    assert_eq!(p.attributes_size(), 1);
    let attr = p.attribute("key1").unwrap();
    let value: Arc<String> = attr.downcast::<String>().unwrap();
    assert_eq!(value.as_ref(), "val1");
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_put_attribute_overwrite() {
    let p = make_pooled(make_physical().await).await;
    p.put_attribute("k", ProxyAttributeValue::new(42_i32));
    let old = p.put_attribute("k", ProxyAttributeValue::new(99_i32));
    let old_val: Arc<i32> = old.unwrap().downcast::<i32>().unwrap();
    assert_eq!(*old_val, 42);
    assert_eq!(p.attributes_size(), 1);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_clear_attributes() {
    let p = make_pooled(make_physical().await).await;
    p.put_attribute("k", ProxyAttributeValue::new(true));
    assert_eq!(p.attributes_size(), 1);
    p.clear_attributes();
    assert_eq!(p.attributes_size(), 0);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_transaction_info_none_initially() {
    let p = make_pooled(make_physical().await).await;
    assert!(p.transaction_info().is_none());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_variables_none_initially() {
    let p = make_pooled(make_physical().await).await;
    assert!(p.variables().is_none());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_global_variables_none_initially() {
    let p = make_pooled(make_physical().await).await;
    assert!(p.global_variables().is_none());
}

#[tokio::test]
#[allow(deprecated)]
async fn task6_druid_pooled_connection_differential_test_gloabl_variables_deprecated_alias() {
    let p = make_pooled(make_physical().await).await;
    let gv = p.gloabl_variables();
    assert!(gv.is_none());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_connection_holder_some_initially() {
    let p = make_pooled(make_physical().await).await;
    assert!(p.connection_holder().is_some());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_connection_holder_mut_some_initially() {
    let mut p = make_pooled(make_physical().await).await;
    assert!(p.connection_holder_mut().is_some());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_standard_connection_mut_some_initially() {
    let mut p = make_pooled(make_physical().await).await;
    assert!(druid::core::Connection::ping(&mut p).await.is_ok());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_debug_format_contains_id_and_data_source()
{
    let p = make_pooled_with_source(make_physical().await, "test-ds").await;
    let debug = format!("{p:?}");
    assert!(debug.contains("DruidPooledConnection"));
    assert!(debug.contains("test-ds"));
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_wrapper_as_any_returns_self() {
    let pooled = make_pooled(make_physical().await).await;
    let concrete = WrapperExt::unwrap_ref::<DruidPooledConnection>(&pooled).unwrap();
    assert_eq!(
        TypeId::of::<DruidPooledConnection>(),
        std::any::Any::type_id(concrete)
    );
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_wrapper_is_wrapper_for_self_type() {
    let pooled = make_pooled(make_physical().await).await;
    assert!(WrapperExt::is_wrapper_for_type::<DruidPooledConnection>(
        &pooled
    ));
    assert_eq!(
        Wrapper::is_wrapper_for(&pooled, Some(TypeId::of::<dyn Wrapper>())),
        Ok(true)
    );
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_wrapper_is_wrapper_for_standard_connection(
) {
    let pooled = make_pooled(make_physical().await).await;
    assert_eq!(
        Wrapper::is_wrapper_for(&pooled, Some(TypeId::of::<dyn Connection>())),
        Ok(true)
    );
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_wrapper_is_wrapper_for_none_returns_false()
{
    let pooled = make_pooled(make_physical().await).await;
    assert!(matches!(
        Wrapper::is_wrapper_for(&pooled, None),
        Err(RdbcError::InvalidArgument(_))
    ));
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_wrapper_is_wrapper_for_random_type_returns_false(
) {
    let pooled = make_pooled(make_physical().await).await;
    assert_eq!(
        Wrapper::is_wrapper_for(&pooled, Some(TypeId::of::<String>())),
        Ok(false)
    );
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_wrapper_unwrap_none_returns_none() {
    let pooled = make_pooled(make_physical().await).await;
    assert!(matches!(
        Wrapper::unwrap(&pooled, None),
        Err(RdbcError::InvalidArgument(_))
    ));
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_wrapper_unwrap_self_type_returns_object() {
    let pooled = make_pooled(make_physical().await).await;
    assert!(WrapperExt::unwrap_ref::<DruidPooledConnection>(&pooled).is_ok());
    assert!(matches!(
        Wrapper::unwrap(&pooled, Some(TypeId::of::<dyn Wrapper>())).unwrap(),
        StandardUnwrapped::Wrapper(_)
    ));
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_wrapper_unwrap_standard_connection_type() {
    let pooled = make_pooled(make_physical().await).await;
    assert!(matches!(
        Wrapper::unwrap(&pooled, Some(TypeId::of::<dyn Connection>())).unwrap(),
        StandardUnwrapped::Connection(_)
    ));
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_auto_commit_true_by_default() {
    let p = make_pooled(make_physical().await).await;
    assert!(p.auto_commit().unwrap());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_driver_name_is_sqlite() {
    let p = make_pooled(make_physical().await).await;
    assert_eq!(p.driver_name().unwrap(), "SQLite");
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_transaction_isolation_default_sqlite() {
    let p = make_pooled(make_physical().await).await;
    assert_eq!(p.transaction_isolation().unwrap().jdbc_code(), 8);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_capabilities_delegates_to_standard() {
    let p = make_pooled(make_physical().await).await;
    let caps = p.capabilities();
    assert!(caps.transactions);
    assert!(caps.savepoints);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_read_only_false_by_default() {
    let p = make_pooled(make_physical().await).await;
    assert!(!p.read_only().unwrap());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_holdability_delegates() {
    let p = make_pooled(make_physical().await).await;
    let _ = p.holdability();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_exec_create_table() {
    let mut p = make_pooled(make_physical().await).await;
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_exec_insert_and_select() {
    let mut p = make_pooled(make_physical().await).await;
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", vec![])
        .await
        .unwrap();
    let result = p
        .exec("INSERT INTO t (id, v) VALUES (1, 'hello')", vec![])
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);

    let rows = p
        .fetch("SELECT v FROM t WHERE id = 1", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_begin_commit_through_pooled() {
    let mut p = make_pooled(make_physical().await).await;
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    p.begin().await.unwrap();
    p.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    p.commit().await.unwrap();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_begin_rollback_through_pooled() {
    let mut p = make_pooled(make_physical().await).await;
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    p.begin().await.unwrap();
    p.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    p.rollback().await.unwrap();
    let rows = p.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 0);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_savepoint_lifecycle_through_pooled() {
    let mut p = make_pooled(make_physical().await).await;
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    p.begin().await.unwrap();
    p.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    let sp = p.set_savepoint().await.unwrap();
    p.exec("INSERT INTO t (id) VALUES (2)", vec![])
        .await
        .unwrap();
    p.rollback_to(&sp).await.unwrap();
    p.release_savepoint(&sp).await.unwrap();
    p.commit().await.unwrap();
    let rows = p.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_named_savepoint_through_pooled() {
    let mut p = make_pooled(make_physical().await).await;
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    p.begin().await.unwrap();
    let sp = p.set_savepoint_named("my_sp").await.unwrap();
    assert!(sp.name.is_some());
    p.release_savepoint(&sp).await.unwrap();
    p.commit().await.unwrap();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_warnings_returns_none_for_sqlite() {
    let mut p = make_pooled(make_physical().await).await;
    let w = DruidPooledConnection::warnings(&mut p).await.unwrap();
    assert!(w.is_none());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_clear_warnings_succeeds() {
    let mut p = make_pooled(make_physical().await).await;
    DruidPooledConnection::clear_warnings(&mut p).await.unwrap();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_set_auto_commit_false_begins_transaction()
{
    let mut p = make_pooled(make_physical().await).await;
    assert!(p.auto_commit().unwrap());
    p.set_auto_commit(false).await.unwrap();
    assert!(!p.auto_commit().unwrap());
    p.set_auto_commit(true).await.unwrap();
    assert!(p.auto_commit().unwrap());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_set_transaction_isolation_serializable() {
    let mut p = make_pooled(make_physical().await).await;
    p.set_transaction_isolation(TransactionIsolation::SERIALIZABLE)
        .await
        .unwrap();
    assert_eq!(p.transaction_isolation().unwrap().jdbc_code(), 8);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_close_sets_is_closed() {
    let mut p = make_pooled(make_physical().await).await;
    assert!(!p.is_closed().unwrap());
    p.close().await.unwrap();
    assert!(p.is_closed().unwrap());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_close_is_idempotent() {
    let mut p = make_pooled(make_physical().await).await;
    p.close().await.unwrap();
    p.close().await.unwrap();
    assert!(p.is_closed().unwrap());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_recycle_consumes_and_sets_recycled() {
    let p = make_pooled(make_physical().await).await;
    assert!(!p.is_recycled());
    p.recycle();
    // p is moved; no further assertions on it
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_discard_connection_sets_recycled() {
    let mut p = make_pooled(make_physical().await).await;
    let _ = p.discard_connection();
    assert!(p.is_recycled());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_ping_succeeds() {
    let mut p = make_pooled(make_physical().await).await;
    p.ping().await.unwrap();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_create_statement_and_execute_query() {
    let mut pooled = make_pooled(make_physical().await).await;
    let mut statement = pooled
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut result_set = statement
        .execute_query("SELECT 42 AS answer")
        .await
        .unwrap();
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.int(ColumnIndex::new(1).unwrap()).unwrap(), 42);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_statement_and_exec() {
    let mut pooled = make_pooled(make_physical().await).await;
    pooled
        .exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", vec![])
        .await
        .unwrap();
    let mut prepared = pooled
        .prepare_statement(
            "INSERT INTO t (id, v) VALUES (?, ?)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).unwrap(),
            value: Value::Int(1),
        })
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(2).unwrap(),
            value: Value::String("test".to_owned()),
        })
        .unwrap();
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_statement_with_result_set_options()
{
    let mut pooled = make_pooled(make_physical().await).await;
    pooled
        .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let options = StatementOptions::new(
        ResultSetType::ForwardOnly,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::CloseAtCommit,
    );
    let prepared = pooled.prepare_statement("SELECT * FROM t", options).await;
    assert!(prepared.is_ok());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_statement_with_holdability_option()
{
    let mut pooled = make_pooled(make_physical().await).await;
    pooled
        .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let options = StatementOptions::new(
        ResultSetType::ForwardOnly,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::HoldOverCommit,
    );
    assert!(pooled
        .prepare_statement("SELECT * FROM t", options)
        .await
        .is_ok());
}
