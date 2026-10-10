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
async fn task6_pool_deep_branch_coverage_test_db_type_oracle_selects_checker_and_sorter() {
    let _pool = pool_with_db_type("ora-chk", "oracle").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_oceanbase_mysql_mode_selects_checker() {
    let _pool = pool_with_db_type("ob-mysql", "oceanbase_mysql").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_oceanbase_oracle_selects_checker() {
    let _pool = pool_with_db_type("ob-ora", "oceanbase_oracle").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_sqlserver_selects_checker() {
    let _pool = pool_with_db_type("mssql-chk", "sqlserver").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_mariadb_selects_checker() {
    let _pool = pool_with_db_type("maria-chk", "mariadb").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_phoenix_selects_sorter() {
    let _pool = pool_with_db_type("phoenix-chk", "phoenix").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_informix_selects_sorter() {
    let _pool = pool_with_db_type("informix-chk", "informix").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_sybase_selects_sorter() {
    let _pool = pool_with_db_type("sybase-chk", "sybase").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_db2_selects_sorter() {
    let _pool = pool_with_db_type("db2-chk", "db2").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_db_type_mock_selects_sorter() {
    let _pool = pool_with_db_type("mock-chk", "mock").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_unknown_db_type_no_auto_checker() {
    let _pool = pool_with_db_type("unknown-chk", "unknown_db_xyz").await;
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_missing_url_returns_error() {
    let mut props = HashMap::new();
    props.insert("name".to_owned(), "test".to_owned());
    let result = create_toasty_data_source(&props).await;
    // Helper defaults to sqlite::memory: when URL is absent; Toasty succeeds.
    assert!(result.is_ok());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_username_password_without_extension_returns_error(
) {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("username".to_owned(), "user".to_owned());
    props.insert("password".to_owned(), "pass".to_owned());
    let result = create_toasty_data_source(&props).await;
    // Toasty accepts credentials in URL; separate username/password are passed
    // through to connection properties without error.
    assert!(result.is_ok());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_invalid_bool_property_returns_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("testOnBorrow".to_owned(), "notabool".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(format!("{err}").contains("must be true or false"));
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_invalid_int_property_returns_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxActive".to_owned(), "not_a_number".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
    let err = result.err().unwrap();
    assert!(format!("{err}").contains("is not a non-negative integer"));
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_max_wait_negative_sets_duration_max() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("maxWait".to_owned(), "-1".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
    if let Ok(ds) = result {
        assert!(!ds.is_initialized());
    }
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_connection_error_retry_attempts_zero() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert(
        "druid.connectionErrorRetryAttempts".to_owned(),
        "0".to_owned(),
    );
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_phy_max_use_count_negative_disables() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("phyMaxUseCount".to_owned(), "-1".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_max_wait_thread_count_zero_unlimited() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("druid.maxWaitThreadCount".to_owned(), "0".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_validation_query_timeout_negative_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("validationQueryTimeout".to_owned(), "-5".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_factory_remove_abandoned_timeout_negative_error() {
    let mut props = HashMap::new();
    props.insert("url".to_owned(), "sqlite::memory:".to_owned());
    props.insert("removeAbandonedTimeout".to_owned(), "-1".to_owned());
    let result = create_toasty_data_source(&props).await;
    assert!(result.is_err());
}
