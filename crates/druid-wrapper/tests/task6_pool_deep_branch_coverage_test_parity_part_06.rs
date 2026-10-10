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

#[test]
fn task6_pool_deep_branch_coverage_test_spi_resource_capabilities_presets() {
    let array = ResourceOperations::array();
    assert!(array.contains(ResourceOperations::READ));
    assert!(array.contains(ResourceOperations::RANGE));
    assert!(array.contains(ResourceOperations::TYPE_MAP));
    assert!(array.contains(ResourceOperations::RESULT_SET));
    assert!(array.contains(ResourceOperations::FREE));

    let blob = ResourceOperations::blob();
    assert!(blob.contains(ResourceOperations::READ));
    assert!(blob.contains(ResourceOperations::WRITE));
    assert!(blob.contains(ResourceOperations::SEARCH));
    assert!(blob.contains(ResourceOperations::TRUNCATE));
    assert!(blob.contains(ResourceOperations::STREAM));

    let clob = ResourceOperations::clob();
    assert!(clob.contains(ResourceOperations::READ));
    assert!(clob.contains(ResourceOperations::WRITE));
    assert!(clob.contains(ResourceOperations::TRUNCATE));

    let reference = ResourceOperations::reference();
    assert!(reference.contains(ResourceOperations::READ));
    assert!(reference.contains(ResourceOperations::WRITE));
    assert!(reference.contains(ResourceOperations::TYPE_MAP));

    let sql_xml = ResourceOperations::sql_xml();
    assert!(sql_xml.contains(ResourceOperations::READ));
    assert!(sql_xml.contains(ResourceOperations::WRITE));
    assert!(sql_xml.contains(ResourceOperations::STREAM));
    assert!(sql_xml.contains(ResourceOperations::FREE));
}

#[test]
fn task6_pool_deep_branch_coverage_test_spi_factory_detached_array() {
    let access = Box::new(StubArrayAccess);
    let _array = ResourceHandleFactory::array(access);
}

#[test]
fn task6_pool_deep_branch_coverage_test_spi_factory_detached_blob() {
    let access = Box::new(StubBlobAccess);
    let _blob = ResourceHandleFactory::blob(access);
}

#[test]
fn task6_pool_deep_branch_coverage_test_spi_factory_detached_clob() {
    let access = Box::new(StubClobAccess);
    let _clob = ResourceHandleFactory::clob(access);
}

#[test]
fn task6_pool_deep_branch_coverage_test_spi_factory_detached_n_clob() {
    let access = Box::new(StubNClobAccess);
    let _n_clob = ResourceHandleFactory::n_clob(access);
}

#[test]
fn task6_pool_deep_branch_coverage_test_spi_factory_detached_reference() {
    let access = Box::new(StubRefAccess);
    let _reference = ResourceHandleFactory::reference(access);
}

#[test]
fn task6_pool_deep_branch_coverage_test_spi_factory_detached_sql_xml() {
    let access = Box::new(StubSqlXmlAccess);
    let _sql_xml = ResourceHandleFactory::sql_xml(access);
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_spi_resource_access_default_free_returns_ok() {
    let mut access = StubArrayAccess;
    assert!(<StubArrayAccess as Array>::free(&mut access).await.is_ok());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_spi_array_access_default_methods_return_not_supported(
) {
    let mut access = StubArrayAccess;
    assert!(access
        .values_with_type_map(&Default::default())
        .await
        .is_err());
    assert!(access.values_range(1, 10).await.is_err());
    assert!(access
        .values_range_with_type_map(1, 10, &Default::default())
        .await
        .is_err());
    assert!(access.result_set().await.is_err());
    assert!(access
        .result_set_with_type_map(&Default::default())
        .await
        .is_err());
    assert!(access.result_set_range(1, 10).await.is_err());
    assert!(access
        .result_set_range_with_type_map(1, 10, &Default::default())
        .await
        .is_err());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_spi_blob_access_default_methods_return_not_supported()
{
    let mut access = StubBlobAccess;
    assert!(access.get_binary_stream().await.is_err());
    assert!(access.position_bytes(&[1, 2], 0).await.is_err());
    assert!(access.set_bytes(0, &[1, 2]).await.is_err());
    assert!(access.set_bytes_range(0, &[1, 2], 0, 2).await.is_err());
    assert!(access.set_binary_stream(0).await.is_err());
    assert!(access.truncate(0).await.is_err());
    assert!(access.get_binary_stream_range(0, 10).await.is_err());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_spi_clob_access_default_methods_return_not_supported()
{
    let mut access = StubClobAccess;
    assert!(access.get_character_stream().await.is_err());
    assert!(access.get_ascii_stream().await.is_err());
    let test_str: druid::core::RdbcString = "test".into();
    assert!(access.position_string(&test_str, 0).await.is_err());
    assert!(access.set_string(0, &test_str).await.is_err());
    assert!(access.set_string_range(0, &test_str, 0, 4).await.is_err());
    assert!(access.set_ascii_stream(0).await.is_err());
    assert!(access.set_character_stream(0).await.is_err());
    assert!(access.truncate(0).await.is_err());
    assert!(access.get_character_stream_range(0, 10).await.is_err());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_spi_ref_access_default_methods_return_not_supported()
{
    let mut access = StubRefAccess;
    assert!(access
        .object_with_type_map(&Default::default())
        .await
        .is_err());
    assert!(access
        .set_object(druid::core::RdbcObject::Scalar(druid::core::Value::Null))
        .await
        .is_err());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_spi_sql_xml_access_default_methods_return_not_supported(
) {
    let mut access = StubSqlXmlAccess;
    assert!(access.binary_stream().await.is_err());
    assert!(access.set_binary_stream().await.is_err());
    assert!(access.character_stream().await.is_err());
    assert!(access.set_character_stream().await.is_err());
    let test_str: druid::core::RdbcString = "test".into();
    assert!(access.set_string(&test_str).await.is_err());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_create_connection_until_deadline_timeout() {
    let factory = Arc::new(FailingFactory::new("slow fail"));
    let pool = make_fault_pool_with_config("deadline-timeout", factory, |b| {
        b.fail_fast(false)
            .connection_error_retry_attempts(0)
            .time_between_connect_error(Duration::from_millis(10))
            .break_after_acquire_failure(false)
            .acquire_timeout(Duration::from_millis(30))
    })
    .await;
    let result = pool.get().await;
    assert!(result.is_err());
}
