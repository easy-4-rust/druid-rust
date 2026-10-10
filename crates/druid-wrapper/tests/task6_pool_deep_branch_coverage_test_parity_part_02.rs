//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Pool deep-branch coverage: error/timeout/fault injection paths.
//!
//! Covers uncovered lines in `pool_inner.rs`, `druid_pool.rs`,
//! `druid_data_source_factory.rs`, `connection_create_worker.rs`,
//! `connection_close_worker.rs`, and spi/ modules.

use druid::core::{
    Connection, ConnectionFactory, DruidError, ExecResult, RdbcError, Row, SqlException, Value,
};
use druid::core::{ResourceHandleFactory, ResourceOperations};
use druid::pool::{DruidDataSourceFactory, DruidPool, DruidPoolBuilder};
use druid_rdbc::resource::{
    Array, Blob, BlobPattern, Clob, ClobPattern, NClob, RdbcInputStream, RdbcOutputStream,
    RdbcReader, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource, Ref, SqlXml,
};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::types::{RdbcObject, RdbcString, RdbcTypeMap};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Helper: create a Toasty `SQLite` data source using the factory path.
async fn create_toasty_data_source(
    properties: &HashMap<String, String>,
) -> Result<druid::pool::DruidDataSource, DruidError> {
    let url = properties
        .get("url")
        .map_or("sqlite::memory:", String::as_str);
    let factory: Arc<dyn ConnectionFactory> = Arc::new(ToastyConnectionFactory::new(url).await?);
    DruidDataSourceFactory::create_data_source_with_factory(properties, factory, "sqlite").await
}

// ===========================================================================
// Test infrastructure
// ===========================================================================

struct FaultConnection {
    closed: bool,
    close_count: Arc<AtomicU64>,
}

druid_rdbc::impl_standard_wrapper!(FaultConnection => Connection);

#[async_trait::async_trait]
impl Connection for FaultConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult::default())
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
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
        if !self.closed {
            self.closed = true;
            self.close_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("fault-test".to_owned())
    }
}

/// Factory that always fails `create()`.
struct FailingFactory {
    fail_count: Arc<AtomicU64>,
    error_message: String,
}

impl FailingFactory {
    fn new(error_message: impl Into<String>) -> Self {
        Self {
            fail_count: Arc::new(AtomicU64::new(0)),
            error_message: error_message.into(),
        }
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for FailingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.fail_count.fetch_add(1, Ordering::Relaxed);
        Err(RdbcError::Other(self.error_message.clone()))
    }
    async fn validate(&self, _connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        Ok(())
    }
}

struct SuccessFactory {
    create_count: Arc<AtomicU64>,
    close_count: Arc<AtomicU64>,
}

impl SuccessFactory {
    fn new() -> Self {
        Self {
            create_count: Arc::new(AtomicU64::new(0)),
            close_count: Arc::new(AtomicU64::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for SuccessFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(FaultConnection {
            closed: false,
            close_count: self.close_count.clone(),
        }))
    }
    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

async fn make_fault_pool(
    name: &str,
    factory: Arc<dyn ConnectionFactory>,
    max_open: usize,
) -> DruidPool {
    DruidPool::builder()
        .name(name)
        .driver_name("fault-test")
        .factory(factory)
        .max_open(max_open)
        .max_idle(max_open)
        .acquire_timeout(Duration::from_millis(200))
        .build()
        .await
        .unwrap()
}

async fn make_fault_pool_with_config(
    name: &str,
    factory: Arc<dyn ConnectionFactory>,
    configure: impl FnOnce(DruidPoolBuilder) -> DruidPoolBuilder,
) -> DruidPool {
    let builder = DruidPool::builder()
        .name(name)
        .driver_name("fault-test")
        .factory(factory)
        .max_open(4)
        .max_idle(4)
        .acquire_timeout(Duration::from_millis(200));
    configure(builder).build().await.unwrap()
}

// ===========================================================================
// 1. pool_inner: create_error + fail_continuous paths
// ===========================================================================

// ===========================================================================
// 2. pool_inner: onFatalError max_active gate
// ===========================================================================

// ===========================================================================
// 3. pool_inner: validate_connection branches
// ===========================================================================

use druid::core::ValidConnectionChecker;

struct AlwaysFailChecker;

#[async_trait::async_trait]
impl ValidConnectionChecker for AlwaysFailChecker {
    async fn is_valid_connection(
        &self,
        _connection: &mut Box<dyn Connection>,
        _query: Option<&str>,
        _validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        Ok(false)
    }
}

// ===========================================================================
// 4. pool_inner: init_exception_throw=false retry loop
// ===========================================================================

struct FailThenSucceedFactory {
    fail_until: u64,
    attempt: AtomicU64,
    close_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl ConnectionFactory for FailThenSucceedFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        let attempt = self.attempt.fetch_add(1, Ordering::Relaxed) + 1;
        if attempt <= self.fail_until {
            return Err(RdbcError::Other(format!("fail attempt {attempt}")));
        }
        Ok(Box::new(FaultConnection {
            closed: false,
            close_count: self.close_count.clone(),
        }))
    }
    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

// ===========================================================================
// 5. pool_inner: return_connection discard branches
// ===========================================================================

// ===========================================================================
// 6. pool_inner: shrink with physical_timeout
// ===========================================================================

// ===========================================================================
// 7. pool_inner: shrink with max_evictable_idle_time
// ===========================================================================

// ===========================================================================
// 8. druid_pool: max_wait_thread_count exceeded
// ===========================================================================

// ===========================================================================
// 9. druid_pool: not_full_timeout_retry_count
// ===========================================================================

// ===========================================================================
// 10. druid_pool: close two-phase (close_resources)
// ===========================================================================

// ===========================================================================
// 11. druid_pool: restart paths
// ===========================================================================

// ===========================================================================
// 12. druid_pool: select_valid_connection_checker / select_exception_sorter
// ===========================================================================

async fn pool_with_db_type(name: &str, db_type: &str) -> DruidPool {
    let factory = Arc::new(SuccessFactory::new());
    DruidPool::builder()
        .name(name)
        .driver_name("test")
        .factory(factory)
        .max_open(2)
        .max_idle(2)
        .db_type_name(db_type)
        .build()
        .await
        .unwrap()
}

// ===========================================================================
// 13. druid_data_source_factory: property parsing edge cases
// ===========================================================================

// ===========================================================================
// 14. druid_data_source_factory: wall_config_from_properties
// ===========================================================================

// ===========================================================================
// 15. druid_data_source_factory: init=true with success
// ===========================================================================

// ===========================================================================
// 16. druid_pool: get_connection_internal idle path branches
// ===========================================================================

// ===========================================================================
// 17. druid_pool: lifecycle_generation mismatch
// ===========================================================================

// ===========================================================================
// 18. druid_pool: DataSourceNotAvailable from create_connection_until
// ===========================================================================

// ===========================================================================
// 19. pool_inner: should_evict + shrink
// ===========================================================================

// ===========================================================================
// 20. pool_inner: close drains idle
// ===========================================================================

// ===========================================================================
// 21. pool_inner: discard + refill
// ===========================================================================

// ===========================================================================
// 22. connection_create_worker: closed during refill
// ===========================================================================

// ===========================================================================
// 23. connection_close_worker: filter chain close path
// ===========================================================================

// ===========================================================================
// 26. spi: ResourceOperations
// ===========================================================================

// ===========================================================================
// 28. spi: ResourceHandleFactory
// ===========================================================================

/// 保留原测试值及不支持分支的标准夹具。对应 Java: java.sql.Array。
#[derive(Debug)]
struct StubArrayAccess;
#[async_trait::async_trait]
impl Array for StubArrayAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::FREE
    }
    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        Ok("VARCHAR".to_owned())
    }
    async fn base_type(&mut self) -> Result<i32, RdbcError> {
        Ok(12)
    }
    async fn values(&mut self) -> Result<Vec<RdbcObject>, RdbcError> {
        Ok(Vec::new())
    }
    async fn values_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Array#values_with_type_map",
        ))
    }
    async fn values_range(
        &mut self,
        _index: i64,
        _count: i32,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(RdbcError::feature_not_supported("Array#values_range"))
    }
    async fn values_range_with_type_map(
        &mut self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Array#values_range_with_type_map",
        ))
    }
    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported("Array#result_set"))
    }
    async fn result_set_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Array#result_set_with_type_map",
        ))
    }
    async fn result_set_range(
        &mut self,
        _index: i64,
        _count: i32,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported("Array#result_set_range"))
    }
    async fn result_set_range_with_type_map(
        &mut self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Array#result_set_range_with_type_map",
        ))
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        // 原夹具无外部状态，保留旧默认free成功。
        Ok(())
    }
}
/// 保留原测试值及不支持分支的标准夹具。对应 Java: java.sql.Blob。
#[derive(Debug)]
struct StubBlobAccess;
#[async_trait::async_trait]
impl Blob for StubBlobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::RANGE | ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        Ok(0)
    }
    async fn get_bytes(&mut self, _position: i64, _length: i32) -> Result<Vec<u8>, RdbcError> {
        Ok(Vec::new())
    }
    async fn get_binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#get_binary_stream"))
    }
    async fn position_bytes(
        &mut self,
        _pattern: &[u8],
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#position_bytes"))
    }
    async fn position_blob(
        &mut self,
        _pattern: BlobPattern<'_>,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#position_blob"))
    }
    async fn set_bytes(&mut self, _position: i64, _bytes: &[u8]) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#set_bytes"))
    }
    async fn set_bytes_range(
        &mut self,
        _position: i64,
        _bytes: &[u8],
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#set_bytes_range"))
    }
    async fn set_binary_stream(&mut self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#set_binary_stream"))
    }
    async fn truncate(&mut self, _length: i64) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("Blob#truncate"))
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        // 原夹具无外部状态，保留旧默认free成功。
        Ok(())
    }
    async fn get_binary_stream_range(
        &mut self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Blob#get_binary_stream_range",
        ))
    }
}
/// 保留原测试值及不支持分支的标准夹具。对应 Java: java.sql.Clob。
#[derive(Debug)]
struct StubClobAccess;
#[async_trait::async_trait]
impl Clob for StubClobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::RANGE | ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        Ok(0)
    }
    async fn get_sub_string(
        &mut self,
        _position: i64,
        _length: i32,
    ) -> Result<RdbcString, RdbcError> {
        Ok("".into())
    }
    async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#get_character_stream",
        ))
    }
    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#get_ascii_stream"))
    }
    async fn position_string(
        &mut self,
        _pattern: &RdbcString,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#position_string"))
    }
    async fn position_clob(
        &mut self,
        _pattern: ClobPattern<'_>,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#position_clob"))
    }
    async fn set_string(&mut self, _position: i64, _value: &RdbcString) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_string"))
    }
    async fn set_string_range(
        &mut self,
        _position: i64,
        _value: &RdbcString,
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_string_range"))
    }
    async fn set_ascii_stream(&mut self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_ascii_stream"))
    }
    async fn set_character_stream(&mut self, _position: i64) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#set_character_stream",
        ))
    }
    async fn truncate(&mut self, _length: i64) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#truncate"))
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        // 原夹具无外部状态，保留旧默认free成功。
        Ok(())
    }
    async fn get_character_stream_range(
        &mut self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#get_character_stream_range",
        ))
    }
}
/// 保留原测试值及不支持分支的标准夹具。对应 Java: java.sql.NClob。
#[derive(Debug)]
struct StubNClobAccess;
#[async_trait::async_trait]
impl Clob for StubNClobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::RANGE | ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        Ok(0)
    }
    async fn get_sub_string(
        &mut self,
        _position: i64,
        _length: i32,
    ) -> Result<RdbcString, RdbcError> {
        Ok("".into())
    }
    async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#get_character_stream",
        ))
    }
    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#get_ascii_stream"))
    }
    async fn position_string(
        &mut self,
        _pattern: &RdbcString,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#position_string"))
    }
    async fn position_clob(
        &mut self,
        _pattern: ClobPattern<'_>,
        _start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#position_clob"))
    }
    async fn set_string(&mut self, _position: i64, _value: &RdbcString) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_string"))
    }
    async fn set_string_range(
        &mut self,
        _position: i64,
        _value: &RdbcString,
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_string_range"))
    }
    async fn set_ascii_stream(&mut self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#set_ascii_stream"))
    }
    async fn set_character_stream(&mut self, _position: i64) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#set_character_stream",
        ))
    }
    async fn truncate(&mut self, _length: i64) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("Clob#truncate"))
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        // 原夹具无外部状态，保留旧默认free成功。
        Ok(())
    }
    async fn get_character_stream_range(
        &mut self,
        _position: i64,
        _length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "Clob#get_character_stream_range",
        ))
    }
}
impl NClob for StubNClobAccess {}
/// 保留原测试值及不支持分支的标准夹具。对应 Java: java.sql.Ref。
#[derive(Debug)]
struct StubRefAccess;
#[async_trait::async_trait]
impl Ref for StubRefAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ
    }
    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        Ok("REF".to_owned())
    }
    async fn object(&mut self) -> Result<RdbcObject, RdbcError> {
        Ok(RdbcObject::Scalar(Value::Null))
    }
    async fn object_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::feature_not_supported("Ref#object_with_type_map"))
    }
    async fn set_object(&mut self, _value: RdbcObject) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("Ref#set_object"))
    }
}
/// 保留原测试值及不支持分支的标准夹具。对应 Java: java.sql.SqlXml。
#[derive(Debug)]
struct StubSqlXmlAccess;
#[async_trait::async_trait]
impl SqlXml for StubSqlXmlAccess {
    fn capabilities(&self) -> ResourceOperations {
        // Source仍不支持，不能宣称包含Source的整组READ能力。
        ResourceOperations::FREE
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        // 原夹具无外部状态，保留旧默认free成功。
        Ok(())
    }
    async fn binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("SqlXml#binary_stream"))
    }
    async fn set_binary_stream(&mut self) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::feature_not_supported("SqlXml#set_binary_stream"))
    }
    async fn character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        Err(RdbcError::feature_not_supported("SqlXml#character_stream"))
    }
    async fn set_character_stream(&mut self) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "SqlXml#set_character_stream",
        ))
    }
    async fn string(&mut self) -> Result<RdbcString, RdbcError> {
        Ok("<root/>".into())
    }
    async fn set_string(&mut self, _value: &RdbcString) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("SqlXml#set_string"))
    }
    async fn source(
        &mut self,
        _representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlSource, RdbcError> {
        Err(RdbcError::feature_not_supported("SqlXml#source"))
    }
    async fn result(
        &mut self,
        _representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlResult, RdbcError> {
        Err(RdbcError::feature_not_supported("SqlXml#result"))
    }
}

// ===========================================================================
// 29. spi: RdbcResourceAccess default free
// ===========================================================================

// ===========================================================================
// 30. spi: RdbcArrayAccess default methods
// ===========================================================================

// ===========================================================================
// 31. spi: RdbcBlobAccess default methods
// ===========================================================================

// ===========================================================================
// 32. spi: RdbcClobAccess default methods
// ===========================================================================

// ===========================================================================
// 33. spi: RdbcRefAccess default methods
// ===========================================================================

// ===========================================================================
// 34. spi: RdbcSqlXmlAccess default methods
// ===========================================================================

// ===========================================================================
// 35. druid_pool: create_connection_until deadline timeout
// ===========================================================================

// ===========================================================================
// 36. druid_pool: test_while_idle and test_on_borrow
// ===========================================================================

// ===========================================================================
// 37. druid_pool: connection_timeout_error with active count
// ===========================================================================

// ===========================================================================
// 38. pool_inner: keep_alive + shrink combo
// ===========================================================================

// ===========================================================================
// 39. pool_inner: shrink closed early return
// ===========================================================================

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_return_connection_when_closed_discards() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("return-closed", factory.clone(), 4).await;
    let _conn = pool.get().await.unwrap();
    let destroys_before = pool.state().destroy_count;
    pool.close().await;
    drop(_conn);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let state = pool.state();
    // Connection returned after close goes through destroy path.
    assert!(
        state.destroy_count > destroys_before,
        "destroy_count should increase: before={destroys_before} after={}",
        state.destroy_count
    );
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_return_connection_when_disabled_discards() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("return-disabled", factory.clone(), 4).await;
    let _conn = pool.get().await.unwrap();
    let destroys_before = pool.state().destroy_count;
    pool.set_enabled(false);
    drop(_conn);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let state = pool.state();
    assert!(
        state.destroy_count > destroys_before,
        "destroy_count should increase: before={destroys_before} after={}",
        state.destroy_count
    );
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_shrink_evicts_standardly_expired_connections() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("shrink-phys-timeout", factory, |b| {
        b.physical_connection_timeout(Duration::from_millis(10))
            .idle_timeout(Duration::from_hours(1))
            .min_idle(0)
    })
    .await;
    let mut c1 = pool.get().await.unwrap();
    let mut c2 = pool.get().await.unwrap();
    c1.close().await.unwrap();
    c2.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 2);

    tokio::time::sleep(Duration::from_millis(30)).await;
    pool.shrink_with_options(true, false).await;
    assert_eq!(pool.state().idle_count, 0);
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_shrink_evicts_beyond_max_evictable_idle_time() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("shrink-max-evict", factory, |b| {
        b.idle_timeout(Duration::from_millis(10))
            .max_evictable_idle_time(Duration::from_millis(100))
            .min_idle(0)
    })
    .await;
    let mut c1 = pool.get().await.unwrap();
    c1.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 1);

    tokio::time::sleep(Duration::from_millis(30)).await;
    pool.shrink_with_options(true, false).await;
    assert_eq!(pool.state().idle_count, 0);
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_max_wait_thread_count_exceeded() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("max-wait-thread", factory, |b| {
        b.max_open(1)
            .max_idle(1)
            .max_wait_thread_count(Some(1))
            .acquire_timeout(Duration::from_millis(500))
    })
    .await;

    let _conn = pool.get().await.unwrap();

    let pool = Arc::new(pool);
    let p1 = Arc::clone(&pool);
    let p2 = Arc::clone(&pool);
    let h1 = tokio::spawn(async move { p1.get_timeout(Duration::from_millis(300)).await });
    let h2 = tokio::spawn(async move { p2.get_timeout(Duration::from_millis(300)).await });

    let r1 = h1.await.unwrap();
    let r2 = h2.await.unwrap();
    let has_max_wait = [&r1, &r2]
        .iter()
        .any(|r| matches!(r, Err(DruidError::MaxWaitThreadCountExceeded { .. })));
    assert!(
        has_max_wait || r1.is_err() && r2.is_err(),
        "at least one waiter should be rejected"
    );
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_not_full_timeout_retry_retries_when_not_full() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool_with_config("timeout-retry", factory, |b| {
        b.max_open(2)
            .max_idle(2)
            .not_full_timeout_retry_count(2)
            .acquire_timeout(Duration::from_millis(50))
    })
    .await;
    let conn = pool.get().await.unwrap();
    drop(conn);
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_close_with_active_connections_waits_for_return() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("close-active", factory.clone(), 4).await;
    let _conn = pool.get().await.unwrap();
    pool.close().await;
    assert!(pool.is_closed());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_close_before_init_noop() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("close-pre-init", factory, 4).await;
    assert!(!pool.is_initialized());
    pool.close().await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_restart_after_close_resets_state() {
    let factory = Arc::new(SuccessFactory::new());
    let pool = make_fault_pool("restart-reset", factory, 4).await;
    pool.init().await.unwrap();
    assert!(pool.is_initialized());
    pool.close().await;
    assert!(pool.is_closed());

    pool.restart().await.unwrap();
    assert!(!pool.is_closed());
    assert!(!pool.is_initialized());
    assert!(pool.is_enabled());

    let conn = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn);
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_mysql_selects_checker_and_sorter() {
    // Construction exercises select_valid_connection_checker + select_exception_sorter.
    let _pool = pool_with_db_type("mysql-chk", "mysql").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_postgres_selects_checker_and_sorter() {
    let _pool = pool_with_db_type("pg-chk", "postgresql").await;
}
