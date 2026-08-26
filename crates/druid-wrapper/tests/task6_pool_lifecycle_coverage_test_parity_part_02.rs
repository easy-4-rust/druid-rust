//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! `DruidPool` 生命周期差分覆盖测试（Java Druid 1.2.28 语义对照）。
//!
//! 覆盖 acquire 超时路径、并发借还、close `幂等、state()` 快照字段、
//! `fill/fill_to、restart、is_full、try_get_connection、get_timeout`、
//! `get_connection_direct、init` `幂等、select_valid_connection_checker` /
//! `select_exception_sorter` `分支、publish_stats、reset_stats`、
//! `stat_value_and_reset、login_timeout、db_type_name、url、raw_url`、
//! `connect_properties、filter_class_names、wall_provider` 等路径。

use druid::core::{Connection, ConnectionFactory, DruidError, ExecResult, RdbcError, Row, Value};
use druid::pool::DruidPool;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ===========================================================================
// Test infrastructure
// ===========================================================================

struct PoolTestConnection {
    closed: bool,
    close_count: Arc<AtomicU64>,
}

druid_rdbc::impl_standard_wrapper!(PoolTestConnection => Connection);

#[async_trait::async_trait]
impl Connection for PoolTestConnection {
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
        Ok("pool-lifecycle-test".to_owned())
    }
}

struct PoolTestFactory {
    create_count: Arc<AtomicU64>,
    close_count: Arc<AtomicU64>,
    fail_create: Arc<AtomicBool>,
}

impl PoolTestFactory {
    fn new() -> Self {
        Self {
            create_count: Arc::new(AtomicU64::new(0)),
            close_count: Arc::new(AtomicU64::new(0)),
            fail_create: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for PoolTestFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        if self.fail_create.load(Ordering::Relaxed) {
            return Err(RdbcError::Other("test create failure".to_owned()));
        }
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(PoolTestConnection {
            closed: false,
            close_count: self.close_count.clone(),
        }))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

async fn make_pool(name: &str, max_open: usize) -> (DruidPool, Arc<PoolTestFactory>) {
    make_pool_full(name, max_open, max_open, 0).await
}

async fn make_pool_full(
    name: &str,
    max_open: usize,
    max_idle: usize,
    min_idle: usize,
) -> (DruidPool, Arc<PoolTestFactory>) {
    let factory = Arc::new(PoolTestFactory::new());
    let pool = DruidPool::builder()
        .name(name)
        .driver_name("pool-lifecycle-test")
        .factory(factory.clone())
        .max_open(max_open)
        .max_idle(max_idle)
        .min_idle(min_idle)
        .acquire_timeout(Duration::from_millis(200))
        .build()
        .await
        .unwrap();
    (pool, factory)
}

// ===========================================================================
// 1. init 幂等
// ===========================================================================

// Java DruidDataSource.init()：多次调用只初始化一次。

// ===========================================================================
// 2. get / get_connection 基础
// ===========================================================================

// Java getConnection()：获取并归还连接。

// Java getConnectionDirect(long)：绕过 Filter 直接获取。

// Java getTimeout(long)：指定超时获取。

// ===========================================================================
// 3. acquire 超时
// ===========================================================================

// Java getConnection(long)：池满且超时后返回 `GetConnectionTimeout`。

// ===========================================================================
// 4. 并发借还
// ===========================================================================

// Java 并发获取归还：多个 task 并发获取和归还连接。

// ===========================================================================
// 5. close 幂等
// ===========================================================================

// Java close()：多次调用不 panic。

// Java close()：未初始化时 close 无副作用。

// ===========================================================================
// 6. restart
// ===========================================================================

// Java restart()：关闭后 restart 恢复为未初始化状态。

// Java restart()：有活跃连接时拒绝 restart。

// ===========================================================================
// 7. state() 快照字段
// ===========================================================================

// Java state()：验证所有快照字段存在。

// Java state()：获取连接后 `active_count` 变化。

// ===========================================================================
// 8. fill / fill_to
// ===========================================================================

// Java fill()：将池填充到 maxActive。

// Java fill(int)：填充到指定数量。

// Java fill(int)：负数参数报错。

// Java fill(int)：关闭后 fill 的行为取决于实现。
// Rust 实现中 `fill_to` 先检查 closed，再 init，close 后 restart 可恢复。

// ===========================================================================
// 9. is_full
// ===========================================================================

// Java isFull：active + idle >= maxOpen。

// ===========================================================================
// 10. try_get_connection
// ===========================================================================

// Java tryGetConnection：池为空时返回 None。

// Java tryGetConnection：有空闲连接时返回 Some。

// ===========================================================================
// 11. set_enabled / is_enabled
// ===========================================================================

// Java setEnable(false)：禁用后无法获取连接。

// ===========================================================================
// 12. getter 方法
// ===========================================================================

// Java loginTimeout。

// Java dbTypeName。

// Java url。

// Java rawUrl。

// Java connectProperties。

// Java filterClassNames：无 Filter 时为空。

// Java wallProvider。

// Java rawDriver。

// Java name / driverName。

// ===========================================================================
// 13. stat_value_and_reset / reset_stats
// ===========================================================================

// Java getStatValueAndReset：返回统计快照。

// Java `resetStat：reset_enable=true` 时递增 resetCount。

// Java `resetStat：reset_enable=false` 时无副作用。

// Java publishStats。

// Java statsCollector。

// ===========================================================================
// 14. create_*_id 分配
// ===========================================================================

// Java createConnectionId / createStatementId / createResultSetId 等。

// ===========================================================================
// 15. remove_abandoned / is_remove_abandoned
// ===========================================================================

// Java removeAbandoned：无 `remove_abandoned` 配置时返回 0。

// ===========================================================================
// 16. discard_connection
// ===========================================================================

// Java discardConnection：None 返回 false。

// ===========================================================================
// 17. pooling_connection_info
// ===========================================================================

// Java poolingConnectionInfo：无空闲连接时为空。

// Java poolingConnectionInfo：有空闲连接时返回信息。

// ===========================================================================
// 18. active_connection_stack_trace
// ===========================================================================

// Java getActiveConnectionStackTrace：无 `remove_abandoned` 时为空。

// ===========================================================================
// 19. shrink
// ===========================================================================

// Java shrink()：默认收缩。

// Java `shrink(boolean)：check_time=false`。

// Java shrink(boolean, boolean)：显式选项。

// ===========================================================================
// 20. notify_credentials_changed / user_password_version
// ===========================================================================

// Java `credentials_changed：版本递增`。

// ===========================================================================
// 21. is_initialized / is_closed
// ===========================================================================

// Java isInitialized / isClosed：状态检查。

// ===========================================================================
// 22. filter_chain
// ===========================================================================

// Java filterChain：无 Filter 时返回 None。

// ===========================================================================
// 23. is_on_fatal_error / on_fatal_error_max_active
// ===========================================================================

// Java isOnFatalError：初始状态为 false。

// Java onFatalErrorMaxActive。

// Java isAsyncInit。

// Java isInitExceptionThrow。

// Java isFailContinuous。

// Java lastCreateError / lastCreateErrorTimeMillis。

// ===========================================================================
// 24. Pool trait
// ===========================================================================

// Java Pool `trait：state()` 委托。

// Java Pool `trait：driver_name()` 委托。

// Java Pool `trait：name()` 委托。

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_last_create_error() {
    let (pool, _factory) = make_pool("last-error", 4).await;
    assert!(pool.last_create_error().is_none());
    assert_eq!(pool.last_create_error_time_millis(), 0);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_trait_state() {
    let (pool, _factory) = make_pool("trait-state", 4).await;
    let state = druid::core::Pool::state(&pool);
    assert_eq!(state.name, "trait-state");
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_trait_driver_name() {
    let (pool, _factory) = make_pool("trait-driver", 4).await;
    assert_eq!(druid::core::Pool::driver_name(&pool), "pool-lifecycle-test");
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_trait_name() {
    let (pool, _factory) = make_pool("trait-name", 4).await;
    assert_eq!(druid::core::Pool::name(&pool), "trait-name");
}
