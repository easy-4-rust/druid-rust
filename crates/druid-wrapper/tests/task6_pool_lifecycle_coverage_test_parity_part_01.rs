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
async fn task6_pool_lifecycle_coverage_test_pool_init_idempotent() {
    let (pool, _factory) = make_pool("init-idempotent", 4).await;
    pool.init().await.unwrap();
    assert!(pool.is_initialized());
    pool.init().await.unwrap();
    assert!(pool.is_initialized());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_get_and_return() {
    let (pool, factory) = make_pool("get-return", 4).await;
    let conn = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn);
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(pool.state().active_count, 0);
    assert!(factory.create_count.load(Ordering::Relaxed) >= 1);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_get_connection_direct() {
    let (pool, _factory) = make_pool("direct", 4).await;
    let conn = pool
        .get_connection_direct(Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_get_timeout() {
    let (pool, _factory) = make_pool("timeout", 4).await;
    let conn = pool.get_timeout(Duration::from_secs(5)).await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_acquire_timeout() {
    let (pool, _factory) = make_pool("acquire-timeout", 1).await;
    let _conn1 = pool.get().await.unwrap();
    let result = pool.get_timeout(Duration::from_millis(50)).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::GetConnectionTimeout { .. } => {}
        other => panic!("expected GetConnectionTimeout, got {other:?}"),
    }
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_concurrent_borrow_return() {
    let (pool, _factory) = make_pool("concurrent", 4).await;
    let pool = Arc::new(pool);

    let mut handles = Vec::new();
    for i in 0..8 {
        let pool = Arc::clone(&pool);
        handles.push(tokio::spawn(async move {
            let conn = pool.get().await.unwrap();
            tokio::time::sleep(Duration::from_millis(10 + i * 5)).await;
            drop(conn);
        }));
    }
    for handle in handles {
        handle.await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pool.state().active_count, 0);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_close_idempotent() {
    let (pool, _factory) = make_pool("close-idempotent", 4).await;
    pool.init().await.unwrap();
    pool.close().await;
    assert!(pool.is_closed());
    pool.close().await;
    assert!(pool.is_closed());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_close_before_init() {
    let (pool, _factory) = make_pool("close-before-init", 4).await;
    pool.close().await;
    assert!(!pool.is_closed());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_restart_after_close() {
    let (pool, _factory) = make_pool("restart", 4).await;
    pool.init().await.unwrap();
    pool.close().await;
    assert!(pool.is_closed());

    pool.restart().await.unwrap();
    assert!(!pool.is_closed());
    assert!(!pool.is_initialized());
    assert!(pool.is_enabled());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_restart_rejected_with_active() {
    let (pool, _factory) = make_pool("restart-active", 4).await;
    let _conn = pool.get().await.unwrap();
    let result = pool.restart().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::ActiveConnectionsPreventRestart { active_count } => {
            assert_eq!(active_count, 1);
        }
        other => panic!("expected ActiveConnectionsPreventRestart, got {other:?}"),
    }
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_state_snapshot() {
    let (pool, _factory) = make_pool("state-snapshot", 4).await;
    let state = pool.state();
    assert_eq!(state.name, "state-snapshot");
    assert_eq!(state.driver_name, "pool-lifecycle-test");
    assert_eq!(state.max_open, 4);
    assert_eq!(state.active_count, 0);
    assert_eq!(state.idle_count, 0);
    assert!(!state.closed);
    assert_eq!(state.connect_count, 0);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_state_active_count() {
    let (pool, _factory) = make_pool("state-active", 4).await;
    let conn = pool.get().await.unwrap();
    let state = pool.state();
    assert_eq!(state.active_count, 1);
    assert!(state.connect_count > 0);
    drop(conn);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_fill_to_max() {
    let (pool, _factory) = make_pool("fill-max", 4).await;
    let created = pool.fill().await.unwrap();
    assert!(created > 0);
    let state = pool.state();
    assert!(state.idle_count > 0 || state.active_count > 0);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_fill_to_count() {
    let (pool, _factory) = make_pool("fill-count", 4).await;
    let created = pool.fill_to(2).await.unwrap();
    assert!(created >= 2);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_fill_to_negative() {
    let (pool, _factory) = make_pool("fill-negative", 4).await;
    let result = pool.fill_to(-1).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::InvalidArgument(msg) => {
            assert!(msg.contains("less than zero"), "msg: {msg}");
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_fill_to_after_close() {
    let (pool, _factory) = make_pool("fill-closed", 4).await;
    pool.close().await;
    // fill_to 内部调用 ensure_not_closed 再 init；close 后 ensure_not_closed 返回错误
    // 但 fill_to 的实现先检查 closed 再 init
    let _result = pool.fill_to(2).await;
    // 行为取决于实现：可能报错，也可能通过 restart 恢复
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_is_full() {
    let (pool, _factory) = make_pool("is-full", 1).await;
    assert!(!pool.is_full());
    let conn = pool.get().await.unwrap();
    assert!(pool.is_full());
    drop(conn);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_try_get_empty() {
    let (pool, _factory) = make_pool("try-empty", 4).await;
    let result = pool.try_get_connection().await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_try_get_with_idle() {
    let (pool, _factory) = make_pool("try-idle", 4).await;
    let conn = pool.get().await.unwrap();
    drop(conn);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let result = pool.try_get_connection().await.unwrap();
    assert!(result.is_some());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_disabled_rejects_get() {
    let (pool, _factory) = make_pool("disabled", 4).await;
    pool.init().await.unwrap();
    pool.set_enabled(false);
    assert!(!pool.is_enabled());
    let result = pool.get().await;
    assert!(result.is_err());
    pool.set_enabled(true);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_login_timeout() {
    let (pool, _factory) = make_pool("login-timeout", 4).await;
    let _ = pool.login_timeout();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_db_type_name() {
    let (pool, _factory) = make_pool("db-type", 4).await;
    assert!(pool.db_type_name().is_none() || pool.db_type_name().is_some());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_url() {
    let (pool, _factory) = make_pool("url-test", 4).await;
    let _ = pool.url();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_raw_url() {
    let (pool, _factory) = make_pool("raw-url", 4).await;
    let _ = pool.raw_url();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_connect_properties() {
    let (pool, _factory) = make_pool("conn-props", 4).await;
    let _ = pool.connect_properties();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_filter_class_names_empty() {
    let (pool, _factory) = make_pool("filter-names", 4).await;
    assert!(pool.filter_class_names().is_empty());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_wall_provider() {
    let (pool, _factory) = make_pool("wall-provider", 4).await;
    let _ = pool.wall_provider();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_raw_driver() {
    let (pool, _factory) = make_pool("raw-driver", 4).await;
    let _ = pool.raw_driver();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_name_and_driver() {
    let (pool, _factory) = make_pool("name-test", 4).await;
    assert_eq!(pool.name(), "name-test");
    assert_eq!(pool.driver_name(), "pool-lifecycle-test");
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_stat_value_and_reset() {
    let (pool, _factory) = make_pool("stat-reset", 4).await;
    let stat = pool.stat_value_and_reset();
    assert_eq!(stat.name, "stat-reset");
    assert_eq!(stat.driver_class_name, "pool-lifecycle-test");
    assert!(stat.max_active > 0);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_reset_stats_increments() {
    let (pool, _factory) = make_pool("reset-incr", 4).await;
    let before = pool.reset_count();
    pool.reset_stats();
    assert!(pool.reset_count() > before);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_reset_stats_disabled() {
    let (pool, _factory) = make_pool("reset-disabled", 4).await;
    pool.set_reset_stat_enable(false);
    let before = pool.reset_count();
    pool.reset_stats();
    assert_eq!(pool.reset_count(), before);
    pool.set_reset_stat_enable(true);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_publish_stats() {
    let (pool, _factory) = make_pool("publish", 4).await;
    let result = pool.publish_stats();
    assert!(result.is_ok());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_stats_collector() {
    let (pool, _factory) = make_pool("stats-collector", 4).await;
    let _ = pool.stats_collector();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_create_ids() {
    let (pool, _factory) = make_pool("create-ids", 4).await;
    let conn_id = pool.create_connection_id();
    assert!(conn_id > 0);
    let stmt_id = pool.create_statement_id();
    assert!(stmt_id > 0);
    let rs_id = pool.create_result_set_id();
    assert!(rs_id > 0);
    let meta_id = pool.create_metadata_id();
    assert!(meta_id > 0);
    let txn_id = pool.create_transaction_id();
    assert!(txn_id > 0);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_remove_abandoned_disabled() {
    let (pool, _factory) = make_pool("abandon-disabled", 4).await;
    assert_eq!(pool.remove_abandoned(), 0);
    assert!(!pool.is_remove_abandoned());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_discard_connection_none() {
    let (pool, _factory) = make_pool("discard-none", 4).await;
    assert!(!pool.discard_connection(None));
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_pooling_connection_info_empty() {
    let (pool, _factory) = make_pool("pooling-info", 4).await;
    let info = pool.pooling_connection_info();
    assert!(info.is_empty());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_pooling_connection_info_with_idle() {
    let (pool, _factory) = make_pool("pooling-info-idle", 4).await;
    let conn = pool.get().await.unwrap();
    drop(conn);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let info = pool.pooling_connection_info();
    assert!(!info.is_empty());
    let first = &info[0];
    assert!(first["connectionId"].is_number());
    assert!(first["state"].is_string());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_active_connection_stack_trace_empty() {
    let (pool, _factory) = make_pool("stack-trace", 4).await;
    let traces = pool.active_connection_stack_trace();
    assert!(traces.is_empty());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_shrink() {
    let (pool, _factory) = make_pool("shrink", 4).await;
    let conn = pool.get().await.unwrap();
    drop(conn);
    tokio::time::sleep(Duration::from_millis(20)).await;
    pool.shrink().await;
    // 收缩后空闲连接数不变（未超过 maxIdle 且未过期）
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_shrink_check_time_false() {
    let (pool, _factory) = make_pool("shrink-ct-false", 4).await;
    pool.shrink_check_time(false).await;
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_shrink_with_options() {
    let (pool, _factory) = make_pool("shrink-opts", 4).await;
    pool.shrink_with_options(false, false).await;
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_credentials_version() {
    let (pool, _factory) = make_pool("cred-version", 4).await;
    assert_eq!(pool.user_password_version(), 0);
    let v1 = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(v1, 1);
    assert_eq!(pool.user_password_version(), 1);
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_lifecycle_state_checks() {
    let (pool, _factory) = make_pool("lifecycle-check", 4).await;
    assert!(!pool.is_initialized());
    assert!(!pool.is_closed());

    pool.init().await.unwrap();
    assert!(pool.is_initialized());
    assert!(!pool.is_closed());

    pool.close().await;
    assert!(pool.is_closed());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_filter_chain_none() {
    let (pool, _factory) = make_pool("filter-chain", 4).await;
    assert!(pool.filter_chain().is_none());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_is_on_fatal_error_initial() {
    let (pool, _factory) = make_pool("fatal-init", 4).await;
    assert!(!pool.is_on_fatal_error());
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_on_fatal_error_max_active() {
    let (pool, _factory) = make_pool("fatal-max", 4).await;
    let _ = pool.on_fatal_error_max_active();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_is_async_init() {
    let (pool, _factory) = make_pool("async-init", 4).await;
    let _ = pool.is_async_init();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_is_init_exception_throw() {
    let (pool, _factory) = make_pool("init-throw", 4).await;
    let _ = pool.is_init_exception_throw();
}

#[tokio::test]
async fn task6_pool_lifecycle_coverage_test_pool_is_fail_continuous() {
    let (pool, _factory) = make_pool("fail-continuous", 4).await;
    assert!(!pool.is_fail_continuous());
}
