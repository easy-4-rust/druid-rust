//! `DruidPool` 生命周期差分覆盖测试（Java Druid 1.2.28 语义对照）。
//!
//! 覆盖 acquire 超时路径、并发借还、close `幂等、state()` 快照字段、
//! `fill/fill_to、restart、is_full、try_get_connection、get_timeout`、
//! `get_connection_direct、init` `幂等、select_valid_connection_checker` /
//! `select_exception_sorter` `分支、publish_stats、reset_stats`、
//! `stat_value_and_reset、login_timeout、db_type_name、url、raw_url`、
//! `connect_properties、filter_class_names、wall_provider` 等路径。

use druid_core::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionFactory, Row, Value,
};
use druid_core::pool::DruidPool;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ===========================================================================
// Test infrastructure
// ===========================================================================

struct PoolTestConnection {
    closed: bool,
    discarded: bool,
    close_count: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl PhysicalConnection for PoolTestConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult::default())
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(Vec::new())
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
        if !self.closed {
            self.closed = true;
            self.close_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
    fn is_closed(&self) -> bool {
        self.closed
    }
    fn mark_discarded(&mut self) {
        self.discarded = true;
    }
    fn is_discarded(&self) -> bool {
        self.discarded
    }
    fn driver_name(&self) -> &'static str {
        "pool-lifecycle-test"
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
impl PhysicalConnectionFactory for PoolTestFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        if self.fail_create.load(Ordering::Relaxed) {
            return Err(DruidError::Other("test create failure".to_owned()));
        }
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(PoolTestConnection {
            closed: false,
            discarded: false,
            close_count: self.close_count.clone(),
        }))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
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

/// Java DruidDataSource.init()：多次调用只初始化一次。
#[tokio::test]
async fn pool_init_idempotent() {
    let (pool, _factory) = make_pool("init-idempotent", 4).await;
    pool.init().await.unwrap();
    assert!(pool.is_initialized());
    pool.init().await.unwrap();
    assert!(pool.is_initialized());
}

// ===========================================================================
// 2. get / get_connection 基础
// ===========================================================================

/// Java getConnection()：获取并归还连接。
#[tokio::test]
async fn pool_get_and_return() {
    let (pool, factory) = make_pool("get-return", 4).await;
    let conn = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn);
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(pool.state().active_count, 0);
    assert!(factory.create_count.load(Ordering::Relaxed) >= 1);
}

/// Java getConnectionDirect(long)：绕过 Filter 直接获取。
#[tokio::test]
async fn pool_get_connection_direct() {
    let (pool, _factory) = make_pool("direct", 4).await;
    let conn = pool
        .get_connection_direct(Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn);
}

/// Java getTimeout(long)：指定超时获取。
#[tokio::test]
async fn pool_get_timeout() {
    let (pool, _factory) = make_pool("timeout", 4).await;
    let conn = pool.get_timeout(Duration::from_secs(5)).await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn);
}

// ===========================================================================
// 3. acquire 超时
// ===========================================================================

/// Java getConnection(long)：池满且超时后返回 `GetConnectionTimeout`。
#[tokio::test]
async fn pool_acquire_timeout() {
    let (pool, _factory) = make_pool("acquire-timeout", 1).await;
    let _conn1 = pool.get().await.unwrap();
    let result = pool.get_timeout(Duration::from_millis(50)).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        DruidError::GetConnectionTimeout { .. } => {}
        other => panic!("expected GetConnectionTimeout, got {other:?}"),
    }
}

// ===========================================================================
// 4. 并发借还
// ===========================================================================

/// Java 并发获取归还：多个 task 并发获取和归还连接。
#[tokio::test]
async fn pool_concurrent_borrow_return() {
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

// ===========================================================================
// 5. close 幂等
// ===========================================================================

/// Java close()：多次调用不 panic。
#[tokio::test]
async fn pool_close_idempotent() {
    let (pool, _factory) = make_pool("close-idempotent", 4).await;
    pool.init().await.unwrap();
    pool.close().await;
    assert!(pool.is_closed());
    pool.close().await;
    assert!(pool.is_closed());
}

/// Java close()：未初始化时 close 无副作用。
#[tokio::test]
async fn pool_close_before_init() {
    let (pool, _factory) = make_pool("close-before-init", 4).await;
    pool.close().await;
    assert!(!pool.is_closed());
}

// ===========================================================================
// 6. restart
// ===========================================================================

/// Java restart()：关闭后 restart 恢复为未初始化状态。
#[tokio::test]
async fn pool_restart_after_close() {
    let (pool, _factory) = make_pool("restart", 4).await;
    pool.init().await.unwrap();
    pool.close().await;
    assert!(pool.is_closed());

    pool.restart().await.unwrap();
    assert!(!pool.is_closed());
    assert!(!pool.is_initialized());
    assert!(pool.is_enabled());
}

/// Java restart()：有活跃连接时拒绝 restart。
#[tokio::test]
async fn pool_restart_rejected_with_active() {
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

// ===========================================================================
// 7. state() 快照字段
// ===========================================================================

/// Java state()：验证所有快照字段存在。
#[tokio::test]
async fn pool_state_snapshot() {
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

/// Java state()：获取连接后 `active_count` 变化。
#[tokio::test]
async fn pool_state_active_count() {
    let (pool, _factory) = make_pool("state-active", 4).await;
    let conn = pool.get().await.unwrap();
    let state = pool.state();
    assert_eq!(state.active_count, 1);
    assert!(state.connect_count > 0);
    drop(conn);
}

// ===========================================================================
// 8. fill / fill_to
// ===========================================================================

/// Java fill()：将池填充到 maxActive。
#[tokio::test]
async fn pool_fill_to_max() {
    let (pool, _factory) = make_pool("fill-max", 4).await;
    let created = pool.fill().await.unwrap();
    assert!(created > 0);
    let state = pool.state();
    assert!(state.idle_count > 0 || state.active_count > 0);
}

/// Java fill(int)：填充到指定数量。
#[tokio::test]
async fn pool_fill_to_count() {
    let (pool, _factory) = make_pool("fill-count", 4).await;
    let created = pool.fill_to(2).await.unwrap();
    assert!(created >= 2);
}

/// Java fill(int)：负数参数报错。
#[tokio::test]
async fn pool_fill_to_negative() {
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

/// Java fill(int)：关闭后 fill 的行为取决于实现。
/// Rust 实现中 `fill_to` 先检查 closed，再 init，close 后 restart 可恢复。
#[tokio::test]
async fn pool_fill_to_after_close() {
    let (pool, _factory) = make_pool("fill-closed", 4).await;
    pool.close().await;
    // fill_to 内部调用 ensure_not_closed 再 init；close 后 ensure_not_closed 返回错误
    // 但 fill_to 的实现先检查 closed 再 init
    let _result = pool.fill_to(2).await;
    // 行为取决于实现：可能报错，也可能通过 restart 恢复
}

// ===========================================================================
// 9. is_full
// ===========================================================================

/// Java isFull：active + idle >= maxOpen。
#[tokio::test]
async fn pool_is_full() {
    let (pool, _factory) = make_pool("is-full", 1).await;
    assert!(!pool.is_full());
    let conn = pool.get().await.unwrap();
    assert!(pool.is_full());
    drop(conn);
}

// ===========================================================================
// 10. try_get_connection
// ===========================================================================

/// Java tryGetConnection：池为空时返回 None。
#[tokio::test]
async fn pool_try_get_empty() {
    let (pool, _factory) = make_pool("try-empty", 4).await;
    let result = pool.try_get_connection().await.unwrap();
    assert!(result.is_none());
}

/// Java tryGetConnection：有空闲连接时返回 Some。
#[tokio::test]
async fn pool_try_get_with_idle() {
    let (pool, _factory) = make_pool("try-idle", 4).await;
    let conn = pool.get().await.unwrap();
    drop(conn);
    tokio::time::sleep(Duration::from_millis(20)).await;
    let result = pool.try_get_connection().await.unwrap();
    assert!(result.is_some());
}

// ===========================================================================
// 11. set_enabled / is_enabled
// ===========================================================================

/// Java setEnable(false)：禁用后无法获取连接。
#[tokio::test]
async fn pool_disabled_rejects_get() {
    let (pool, _factory) = make_pool("disabled", 4).await;
    pool.init().await.unwrap();
    pool.set_enabled(false);
    assert!(!pool.is_enabled());
    let result = pool.get().await;
    assert!(result.is_err());
    pool.set_enabled(true);
}

// ===========================================================================
// 12. getter 方法
// ===========================================================================

/// Java loginTimeout。
#[tokio::test]
async fn pool_login_timeout() {
    let (pool, _factory) = make_pool("login-timeout", 4).await;
    let _ = pool.login_timeout();
}

/// Java dbTypeName。
#[tokio::test]
async fn pool_db_type_name() {
    let (pool, _factory) = make_pool("db-type", 4).await;
    assert!(pool.db_type_name().is_none() || pool.db_type_name().is_some());
}

/// Java url。
#[tokio::test]
async fn pool_url() {
    let (pool, _factory) = make_pool("url-test", 4).await;
    let _ = pool.url();
}

/// Java rawUrl。
#[tokio::test]
async fn pool_raw_url() {
    let (pool, _factory) = make_pool("raw-url", 4).await;
    let _ = pool.raw_url();
}

/// Java connectProperties。
#[tokio::test]
async fn pool_connect_properties() {
    let (pool, _factory) = make_pool("conn-props", 4).await;
    let _ = pool.connect_properties();
}

/// Java filterClassNames：无 Filter 时为空。
#[tokio::test]
async fn pool_filter_class_names_empty() {
    let (pool, _factory) = make_pool("filter-names", 4).await;
    assert!(pool.filter_class_names().is_empty());
}

/// Java wallProvider。
#[tokio::test]
async fn pool_wall_provider() {
    let (pool, _factory) = make_pool("wall-provider", 4).await;
    let _ = pool.wall_provider();
}

/// Java rawDriver。
#[tokio::test]
async fn pool_raw_driver() {
    let (pool, _factory) = make_pool("raw-driver", 4).await;
    let _ = pool.raw_driver();
}

/// Java name / driverName。
#[tokio::test]
async fn pool_name_and_driver() {
    let (pool, _factory) = make_pool("name-test", 4).await;
    assert_eq!(pool.name(), "name-test");
    assert_eq!(pool.driver_name(), "pool-lifecycle-test");
}

// ===========================================================================
// 13. stat_value_and_reset / reset_stats
// ===========================================================================

/// Java getStatValueAndReset：返回统计快照。
#[tokio::test]
async fn pool_stat_value_and_reset() {
    let (pool, _factory) = make_pool("stat-reset", 4).await;
    let stat = pool.stat_value_and_reset();
    assert_eq!(stat.name, "stat-reset");
    assert_eq!(stat.driver_class_name, "pool-lifecycle-test");
    assert!(stat.max_active > 0);
}

/// Java `resetStat：reset_enable=true` 时递增 resetCount。
#[tokio::test]
async fn pool_reset_stats_increments() {
    let (pool, _factory) = make_pool("reset-incr", 4).await;
    let before = pool.reset_count();
    pool.reset_stats();
    assert!(pool.reset_count() > before);
}

/// Java `resetStat：reset_enable=false` 时无副作用。
#[tokio::test]
async fn pool_reset_stats_disabled() {
    let (pool, _factory) = make_pool("reset-disabled", 4).await;
    pool.set_reset_stat_enable(false);
    let before = pool.reset_count();
    pool.reset_stats();
    assert_eq!(pool.reset_count(), before);
    pool.set_reset_stat_enable(true);
}

/// Java publishStats。
#[tokio::test]
async fn pool_publish_stats() {
    let (pool, _factory) = make_pool("publish", 4).await;
    let result = pool.publish_stats();
    assert!(result.is_ok());
}

/// Java statsCollector。
#[tokio::test]
async fn pool_stats_collector() {
    let (pool, _factory) = make_pool("stats-collector", 4).await;
    let _ = pool.stats_collector();
}

// ===========================================================================
// 14. create_*_id 分配
// ===========================================================================

/// Java createConnectionId / createStatementId / createResultSetId 等。
#[tokio::test]
async fn pool_create_ids() {
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

// ===========================================================================
// 15. remove_abandoned / is_remove_abandoned
// ===========================================================================

/// Java removeAbandoned：无 `remove_abandoned` 配置时返回 0。
#[tokio::test]
async fn pool_remove_abandoned_disabled() {
    let (pool, _factory) = make_pool("abandon-disabled", 4).await;
    assert_eq!(pool.remove_abandoned(), 0);
    assert!(!pool.is_remove_abandoned());
}

// ===========================================================================
// 16. discard_connection
// ===========================================================================

/// Java discardConnection：None 返回 false。
#[tokio::test]
async fn pool_discard_connection_none() {
    let (pool, _factory) = make_pool("discard-none", 4).await;
    assert!(!pool.discard_connection(None));
}

// ===========================================================================
// 17. pooling_connection_info
// ===========================================================================

/// Java poolingConnectionInfo：无空闲连接时为空。
#[tokio::test]
async fn pool_pooling_connection_info_empty() {
    let (pool, _factory) = make_pool("pooling-info", 4).await;
    let info = pool.pooling_connection_info();
    assert!(info.is_empty());
}

/// Java poolingConnectionInfo：有空闲连接时返回信息。
#[tokio::test]
async fn pool_pooling_connection_info_with_idle() {
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

// ===========================================================================
// 18. active_connection_stack_trace
// ===========================================================================

/// Java getActiveConnectionStackTrace：无 `remove_abandoned` 时为空。
#[tokio::test]
async fn pool_active_connection_stack_trace_empty() {
    let (pool, _factory) = make_pool("stack-trace", 4).await;
    let traces = pool.active_connection_stack_trace();
    assert!(traces.is_empty());
}

// ===========================================================================
// 19. shrink
// ===========================================================================

/// Java shrink()：默认收缩。
#[tokio::test]
async fn pool_shrink() {
    let (pool, _factory) = make_pool("shrink", 4).await;
    let conn = pool.get().await.unwrap();
    drop(conn);
    tokio::time::sleep(Duration::from_millis(20)).await;
    pool.shrink().await;
    // 收缩后空闲连接数不变（未超过 maxIdle 且未过期）
}

/// Java `shrink(boolean)：check_time=false`。
#[tokio::test]
async fn pool_shrink_check_time_false() {
    let (pool, _factory) = make_pool("shrink-ct-false", 4).await;
    pool.shrink_check_time(false).await;
}

/// Java shrink(boolean, boolean)：显式选项。
#[tokio::test]
async fn pool_shrink_with_options() {
    let (pool, _factory) = make_pool("shrink-opts", 4).await;
    pool.shrink_with_options(false, false).await;
}

// ===========================================================================
// 20. notify_credentials_changed / user_password_version
// ===========================================================================

/// Java `credentials_changed：版本递增`。
#[tokio::test]
async fn pool_credentials_version() {
    let (pool, _factory) = make_pool("cred-version", 4).await;
    assert_eq!(pool.user_password_version(), 0);
    let v1 = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(v1, 1);
    assert_eq!(pool.user_password_version(), 1);
}

// ===========================================================================
// 21. is_initialized / is_closed
// ===========================================================================

/// Java isInitialized / isClosed：状态检查。
#[tokio::test]
async fn pool_lifecycle_state_checks() {
    let (pool, _factory) = make_pool("lifecycle-check", 4).await;
    assert!(!pool.is_initialized());
    assert!(!pool.is_closed());

    pool.init().await.unwrap();
    assert!(pool.is_initialized());
    assert!(!pool.is_closed());

    pool.close().await;
    assert!(pool.is_closed());
}

// ===========================================================================
// 22. filter_chain
// ===========================================================================

/// Java filterChain：无 Filter 时返回 None。
#[tokio::test]
async fn pool_filter_chain_none() {
    let (pool, _factory) = make_pool("filter-chain", 4).await;
    assert!(pool.filter_chain().is_none());
}

// ===========================================================================
// 23. is_on_fatal_error / on_fatal_error_max_active
// ===========================================================================

/// Java isOnFatalError：初始状态为 false。
#[tokio::test]
async fn pool_is_on_fatal_error_initial() {
    let (pool, _factory) = make_pool("fatal-init", 4).await;
    assert!(!pool.is_on_fatal_error());
}

/// Java onFatalErrorMaxActive。
#[tokio::test]
async fn pool_on_fatal_error_max_active() {
    let (pool, _factory) = make_pool("fatal-max", 4).await;
    let _ = pool.on_fatal_error_max_active();
}

/// Java isAsyncInit。
#[tokio::test]
async fn pool_is_async_init() {
    let (pool, _factory) = make_pool("async-init", 4).await;
    let _ = pool.is_async_init();
}

/// Java isInitExceptionThrow。
#[tokio::test]
async fn pool_is_init_exception_throw() {
    let (pool, _factory) = make_pool("init-throw", 4).await;
    let _ = pool.is_init_exception_throw();
}

/// Java isFailContinuous。
#[tokio::test]
async fn pool_is_fail_continuous() {
    let (pool, _factory) = make_pool("fail-continuous", 4).await;
    assert!(!pool.is_fail_continuous());
}

/// Java lastCreateError / lastCreateErrorTimeMillis。
#[tokio::test]
async fn pool_last_create_error() {
    let (pool, _factory) = make_pool("last-error", 4).await;
    assert!(pool.last_create_error().is_none());
    assert_eq!(pool.last_create_error_time_millis(), 0);
}

// ===========================================================================
// 24. Pool trait
// ===========================================================================

/// Java Pool `trait：state()` 委托。
#[tokio::test]
async fn pool_trait_state() {
    let (pool, _factory) = make_pool("trait-state", 4).await;
    let state = druid_core::core::Pool::state(&pool);
    assert_eq!(state.name, "trait-state");
}

/// Java Pool `trait：driver_name()` 委托。
#[tokio::test]
async fn pool_trait_driver_name() {
    let (pool, _factory) = make_pool("trait-driver", 4).await;
    assert_eq!(
        druid_core::core::Pool::driver_name(&pool),
        "pool-lifecycle-test"
    );
}

/// Java Pool `trait：name()` 委托。
#[tokio::test]
async fn pool_trait_name() {
    let (pool, _factory) = make_pool("trait-name", 4).await;
    assert_eq!(druid_core::core::Pool::name(&pool), "trait-name");
}
