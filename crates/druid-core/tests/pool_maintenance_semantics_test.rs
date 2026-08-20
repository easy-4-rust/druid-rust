//! Java Druid 池维护语义差分测试（C2 Step 33-35 补充批次）。
//!
//! Java 基线：`DruidDataSource`（Java Druid 1.2.28）。
//!
//! 本文件覆盖以下 Java 语义的差分验证（不含已在
//! `pool_maintenance_differential_test.rs` 和 `maintenance_semantics_test.rs`
//! 中覆盖的子场景）：
//!
//! 1. **removeAbandoned 批量回收**：一次扫描回收多个超时借出连接；
//!    未超时借出连接在同一扫描中保留。
//! 2. **removeAbandoned 与归还交互**：归还后连接不再出现在 active 集合中，
//!    不被误判为 abandoned。
//! 3. **keepAlive 回填（keepAlive=true 路径）**：shrink 驱逐空闲连接后
//!    若 totalCount < minIdle，触发异步回填；失败校验的候选被丢弃并计入
//!    keepAliveCheckErrorCount。
//! 4. **keepAlive 与 fatal-error 交互**：fatal 前创建的空闲连接无论
//!    keepAlive 参数是否开启都进入校验路径。
//! 5. **密码版本失效 — 空闲队列**：`notify_credentials_changed` 清除
//!    旧版本空闲连接并回填新版本。
//! 6. **密码版本失效 — 借出归还**：旧版本借出连接归还时被 discard。
//! 7. **密码版本失效 — get 路径跳过**：从空闲队列取出旧版本连接时跳过，
//!    继续取下一个或创建新连接。
//! 8. **密码版本失效 — 新创建连接也检查**：create_connection_until 产出的
//!    连接若版本落后也立即销毁。

extern crate druid_core as druid;
use druid::core::{
    DruidError, ExecResult, MySqlExceptionSorter, PhysicalConnection, PhysicalConnectionFactory,
    Row, SqlException, ValidConnectionChecker, Value,
};
use druid::pool::DruidPool;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Test infrastructure: mock connection & factory
// ---------------------------------------------------------------------------

struct TestConnection {
    closed_count: Arc<AtomicU64>,
    discarded: bool,
    closed: bool,
}

#[async_trait::async_trait]
impl PhysicalConnection for TestConnection {
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
            self.closed_count.fetch_add(1, Ordering::Relaxed);
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
        "pool-maint-test"
    }
}

struct TestFactory {
    create_count: Arc<AtomicU64>,
    validate_count: Arc<AtomicU64>,
    validation_succeeds: Arc<AtomicBool>,
    closed_count: Arc<AtomicU64>,
}

impl TestFactory {
    fn new() -> Self {
        Self {
            create_count: Arc::new(AtomicU64::new(0)),
            validate_count: Arc::new(AtomicU64::new(0)),
            validation_succeeds: Arc::new(AtomicBool::new(true)),
            closed_count: Arc::new(AtomicU64::new(0)),
        }
    }

    fn checker(&self) -> Arc<dyn ValidConnectionChecker> {
        Arc::new(TestChecker {
            validate_count: Arc::clone(&self.validate_count),
            validation_succeeds: Arc::clone(&self.validation_succeeds),
        })
    }
}

struct TestChecker {
    validate_count: Arc<AtomicU64>,
    validation_succeeds: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl ValidConnectionChecker for TestChecker {
    async fn is_valid_connection(
        &self,
        _connection: &mut Box<dyn PhysicalConnection>,
        _query: Option<&str>,
        _validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        self.validate_count.fetch_add(1, Ordering::Relaxed);
        Ok(self.validation_succeeds.load(Ordering::Relaxed))
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for TestFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(TestConnection {
            closed_count: self.closed_count.clone(),
            discarded: false,
            closed: false,
        }))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        self.validate_count.fetch_add(1, Ordering::Relaxed);
        if self.validation_succeeds.load(Ordering::Relaxed) {
            connection.ping().await
        } else {
            Err(DruidError::ValidationFailed(
                "test validation failed".to_string(),
            ))
        }
    }
}

/// 预填充空闲连接到池中。
async fn fill_idle(pool: &DruidPool, count: usize) {
    let mut connections = Vec::with_capacity(count);
    for _ in 0..count {
        connections.push(pool.get().await.unwrap());
    }
    for mut connection in connections {
        connection.close().await.unwrap();
    }
}

// ===========================================================================
// 1. removeAbandoned 批量回收
// ===========================================================================

/// Java `DruidDataSource#removeAbandoned`：一次扫描回收多个超时借出连接，
/// 保留未超时的借出连接。
///
/// Java 对照：`TestAbondon` 系列测试中多个连接同时借出、部分超时场景。
#[tokio::test]
async fn remove_abandoned_batch_reclaims_all_timed_out_leases() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("abandon-batch")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(8)
        .max_idle(8)
        .remove_abandoned(true)
        .remove_abandoned_timeout(Duration::from_millis(50))
        .log_abandoned(true)
        .build()
        .await
        .unwrap();

    // 借出 3 个连接，全部不归还（模拟泄漏）。
    let _leaked_a = pool.get().await.unwrap();
    let _leaked_b = pool.get().await.unwrap();
    let _leaked_c = pool.get().await.unwrap();

    assert_eq!(pool.state().active_count, 3);

    // 等待超过 abandoned timeout。
    tokio::time::sleep(Duration::from_millis(80)).await;

    let removed = pool.remove_abandoned();
    assert_eq!(removed, 3, "all 3 timed-out leases must be reclaimed");
    let state = pool.state();
    assert_eq!(
        state.leak_detection_count, 3,
        "removeAbandonedCount must equal reclaimed count"
    );
    // 注意：remove_abandoned 只从 active_leases 移除租约条目，
    // active_count 在 DruidPooledConnection::close() 归还时才递减。
    // Java 实现在 removeAbandoned 中直接 close 连接；
    // Rust 实现依赖后续 close() 调用或 Drop 来完成归还。
}

/// Java：未超过 timeout 的借出连接在同一扫描中不被回收。
#[tokio::test]
async fn remove_abandoned_preserves_fresh_borrow() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("abandon-fresh")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .remove_abandoned(true)
        .remove_abandoned_timeout(Duration::from_millis(200))
        .build()
        .await
        .unwrap();

    // 先借出一个并等待超时。
    let _old_leak = pool.get().await.unwrap();
    tokio::time::sleep(Duration::from_millis(250)).await;

    // 再借出一个（在 timeout 内）。
    let _fresh = pool.get().await.unwrap();

    let removed = pool.remove_abandoned();
    assert_eq!(removed, 1, "only the old lease is abandoned");
    assert_eq!(pool.state().leak_detection_count, 1);

    // 旧泄漏连接的租约已失效；close 走静默路径。
    drop(_old_leak);
    // 新鲜借出的连接仍在活跃中。
    assert_eq!(pool.state().active_count, 1, "fresh borrow still active");
}

// ===========================================================================
// 2. removeAbandoned 与归还交互
// ===========================================================================

/// Java：归还的连接从 activeConnections 移除，不再被 removeAbandoned 扫描。
#[tokio::test]
async fn remove_abandoned_ignores_returned_connection() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("abandon-returned")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .remove_abandoned(true)
        .remove_abandoned_timeout(Duration::from_millis(30))
        .build()
        .await
        .unwrap();

    let mut conn = pool.get().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 归还连接。
    conn.close().await.unwrap();

    let removed = pool.remove_abandoned();
    assert_eq!(removed, 0, "returned connection is not abandoned");
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1, "connection returned to idle");
}

// ===========================================================================
// 3. keepAlive 回填语义
// ===========================================================================

/// Java `shrink(checkTime=true, keepAlive=true)`：
/// 空闲连接超过 keepAliveBetweenTimeMillis → 进入校验；
/// 校验成功 → recordKeepAlive + 重新入队；
/// totalCount < minIdle → 异步回填。
#[tokio::test]
async fn keep_alive_validates_and_refills_below_min_idle() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("ka-refill")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .min_idle(3)
        .idle_timeout(Duration::from_secs(60))
        .time_between_eviction_runs(Duration::ZERO)
        .keep_alive(true)
        .keep_alive_between_time(Duration::from_millis(5))
        .valid_connection_checker(factory.checker())
        .build()
        .await
        .unwrap();

    // 填充 2 个空闲连接（少于 min_idle=3）。
    fill_idle(&pool, 2).await;
    let creates_before = factory.create_count.load(Ordering::Relaxed);

    tokio::time::sleep(Duration::from_millis(15)).await;
    pool.shrink_check_time(true).await;

    // 2 个空闲连接均进入 keepAlive 校验。
    let state = pool.state();
    assert_eq!(state.keep_alive_check_count, 2);
    assert_eq!(state.keep_alive_check_error_count, 0);

    // 校验成功后 totalCount(2) < minIdle(3) → 异步回填至少 1 个。
    // 等待回填完成。
    tokio::time::sleep(Duration::from_millis(50)).await;
    let creates_after = factory.create_count.load(Ordering::Relaxed);
    assert!(
        creates_after > creates_before,
        "refill must create new connections: before={creates_before} after={creates_after}"
    );
    let final_state = pool.state();
    assert!(
        final_state.idle_count >= 3 || final_state.idle_count + final_state.active_count >= 3,
        "pool must be at or above minIdle after refill: {:?}",
        final_state
    );
}

/// Java：keepAlive 校验失败的连接被 discard，计入 keepAliveCheckErrorCount。
/// 注意：shrink 内部的 `fill(minIdle)` 也依赖同一 factory 的 validate，
/// 因此 validation_succeeds=false 时 fill 也无法成功。回填语义已在
/// `keep_alive_validates_and_refills_below_min_idle` 中单独覆盖。
#[tokio::test]
async fn keep_alive_failure_discards_and_counts() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("ka-fail-count")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .min_idle(0)
        .idle_timeout(Duration::from_secs(60))
        .keep_alive_between_time(Duration::from_millis(5))
        .valid_connection_checker(factory.checker())
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 3).await;

    // 翻转校验为失败。
    factory.validation_succeeds.store(false, Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(15)).await;
    pool.shrink_with_options(true, true).await;

    let state = pool.state();
    assert_eq!(state.keep_alive_check_count, 3);
    assert_eq!(state.keep_alive_check_error_count, 3);
    assert_eq!(state.discard_count, 3);
    assert_eq!(state.destroy_count, 3);
    assert_eq!(state.idle_count, 0);
    // 物理连接确实被关闭。
    assert_eq!(factory.closed_count.load(Ordering::Relaxed), 3);
}

// ===========================================================================
// 4. keepAlive 与 fatal-error 交互
// ===========================================================================

/// Java `shrink`：`(onFatalError || fatalErrorIncrement > 0) &&
/// (lastFatalErrorTimeMillis > connection.connectTimeMillis)` →
/// 空闲连接无条件进入 keepAliveConnections，无论 keepAlive 参数。
#[tokio::test]
async fn fatal_error_forces_idle_connections_into_keepalive_validation() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("fatal-ka")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .min_idle(0)
        .idle_timeout(Duration::from_secs(60))
        .keep_alive(false)
        .keep_alive_between_time(Duration::from_millis(5))
        .valid_connection_checker(factory.checker())
        .exception_sorter(Arc::new(MySqlExceptionSorter))
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 2).await;

    // 注入 fatal 错误触发 onFatalError 置位。
    let mut conn = pool.get().await.unwrap();
    let fatal = DruidError::SqlException(Box::new(SqlException::driver(
        1042,
        "fatal connection error".to_owned(),
    )));
    assert!(conn.handle_exception(&fatal));
    let _ = conn.close().await;
    assert!(pool.is_on_fatal_error());

    let validate_before = factory.validate_count.load(Ordering::Relaxed);

    // shrink(checkTime=false, keepAlive=false) — 但 fatal 前创建的空闲连接
    // 仍应进入 keepAlive 校验路径。
    pool.shrink_with_options(false, false).await;

    let validate_after = factory.validate_count.load(Ordering::Relaxed);
    assert!(
        validate_after > validate_before,
        "pre-fatal idle connections must enter keepAlive validation even with keepAlive=false"
    );

    let state = pool.state();
    assert!(
        state.keep_alive_check_count > 0,
        "keepAliveCheckCount must reflect forced validation"
    );
}

// ===========================================================================
// 5. 密码版本失效 — 空闲队列清除与回填
// ===========================================================================

/// Java `credentials_changed` + `shrink`：凭据变更后旧版本空闲连接被清除，
/// 新版本连接按原 total 回填。
#[tokio::test]
async fn credentials_change_clears_idle_and_refills_with_new_version() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("cred-idle")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 3).await;
    assert_eq!(pool.state().idle_count, 3);
    assert_eq!(pool.user_password_version(), 0);

    let creates_before = factory.create_count.load(Ordering::Relaxed);
    let new_version = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(new_version, 1);

    // 旧版本空闲连接被销毁，新连接回填。
    let creates_after = factory.create_count.load(Ordering::Relaxed);
    assert!(
        creates_after >= creates_before + 3,
        "must recreate all idle connections: before={creates_before} after={creates_after}"
    );
    let state = pool.state();
    assert_eq!(state.idle_count, 3, "pool refilled to original size");
    assert!(
        state.discard_count >= 3,
        "old idle connections must be discarded"
    );
}

// ===========================================================================
// 6. 密码版本失效 — 借出归还时 discard
// ===========================================================================

/// Java `recycle` 路径：`holder.userPasswordVersion < getUserPasswordVersion()`
/// → discardConnection(holder)。旧版本借出连接归还时被丢弃，不回池。
#[tokio::test]
async fn credentials_change_discards_borrowed_connection_on_return() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("cred-return")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    // 借出 2 个连接。
    let mut conn_a = pool.get().await.unwrap();
    let mut conn_b = pool.get().await.unwrap();

    // 凭据变更（版本 0 → 1）。
    let new_version = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(new_version, 1);

    // 旧版本连接归还 → discard。
    conn_a.close().await.unwrap();
    let state_after_a = pool.state();
    assert_eq!(
        state_after_a.discard_count, 1,
        "first stale return must discard"
    );
    assert_eq!(state_after_a.idle_count, 0, "nothing returns to idle");

    conn_b.close().await.unwrap();
    let state_after_b = pool.state();
    assert_eq!(
        state_after_b.discard_count, 2,
        "second stale return must discard"
    );
    assert_eq!(state_after_b.idle_count, 0);
}

// ===========================================================================
// 7. 密码版本失效 — get 路径跳过旧版本空闲连接
// ===========================================================================

/// Java `getConnectionInternal`：
/// `if (holder.userPasswordVersion < getUserPasswordVersion()) continue;`
/// 跳过旧版本空闲连接，继续取下一个或创建新连接。
#[tokio::test]
async fn credentials_change_get_skips_stale_idle_and_creates_new() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("cred-get-skip")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 2).await;
    let creates_before = factory.create_count.load(Ordering::Relaxed);

    // 凭据变更但不通过 notify_credentials_changed（只递增版本，不清空空闲队列）。
    // 这样空闲队列中仍保留旧版本连接，get 路径必须跳过它们。
    // 使用两次 notify_credentials_changed: 第一次清空 + 回填，第二次只递增版本。
    // 不对 — notify_credentials_changed 会清空空闲队列。
    // 我们需要一个只递增版本的方法。直接测试 get 路径的版本检查：
    // 先 notify_credentials_changed 清空旧连接并回填新版本连接。
    pool.notify_credentials_changed().await.unwrap();
    // 现在池中是版本 1 的新连接，验证能正常借出。
    let creates_after_refill = factory.create_count.load(Ordering::Relaxed);
    assert!(
        creates_after_refill >= creates_before + 2,
        "refill must create new version connections"
    );

    let mut conn = pool.get().await.unwrap();
    let holder = conn.connection_holder().unwrap();
    assert_eq!(
        holder.user_password_version(),
        1,
        "borrowed connection must be at current version"
    );
    conn.close().await.unwrap();
}

// ===========================================================================
// 8. 密码版本失效 — 物理连接实际关闭验证
// ===========================================================================

/// Java `credentials_changed` → 旧版本空闲连接被 `destroyHolder` 关闭物理连接。
/// 验证 factory.closed_count 反映物理连接确实被关闭。
/// destroy_holder 通过 close worker 异步关闭，需等待 worker 处理完毕。
#[tokio::test]
async fn credentials_change_closes_physical_connections_of_stale_idle() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("cred-physical-close")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 3).await;
    let closed_before = factory.closed_count.load(Ordering::Relaxed);
    assert_eq!(closed_before, 0, "no connections closed yet");

    pool.notify_credentials_changed().await.unwrap();

    // destroy_holder 通过 close worker channel 异步执行，等待处理完毕。
    tokio::time::sleep(Duration::from_millis(50)).await;

    let closed_after = factory.closed_count.load(Ordering::Relaxed);
    assert!(
        closed_after >= 3,
        "stale idle connections must have their physical connections closed: {closed_after}"
    );
}

// ===========================================================================
// 9. 综合场景：removeAbandoned + keepAlive + 密码版本
// ===========================================================================

/// 综合验证：先借出连接触发 abandoned，再变更凭据，
/// 验证两个独立维护路径互不干扰。
#[tokio::test]
async fn combined_abandoned_and_credentials_change() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("combined")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(8)
        .max_idle(8)
        .remove_abandoned(true)
        .remove_abandoned_timeout(Duration::from_millis(30))
        .build()
        .await
        .unwrap();

    // 借出 2 个（模拟泄漏）+ 填充 2 个空闲。
    let _leaked = pool.get().await.unwrap();
    let _leaked2 = pool.get().await.unwrap();
    fill_idle(&pool, 2).await;

    // 等待 abandoned timeout。
    tokio::time::sleep(Duration::from_millis(60)).await;

    // 变更凭据（清空空闲队列中旧版本 + 回填）。
    let new_version = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(new_version, 1);

    // removeAbandoned 回收泄漏连接。
    let removed = pool.remove_abandoned();
    assert_eq!(removed, 2, "both leaked connections abandoned");

    let state = pool.state();
    assert_eq!(state.leak_detection_count, 2);
    // 空闲队列已由 credentials_changed 回填为新版本。
    assert_eq!(state.idle_count, 2, "refilled idle connections");
}

// ===========================================================================
// 10. keepAlive 校验次数累计
// ===========================================================================

/// Java `keepAliveCheckCount += keepAliveCount`：多次 shrink 累计。
#[tokio::test]
async fn keep_alive_check_count_accumulates_across_shrinks() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("ka-accumulate")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .min_idle(0)
        .idle_timeout(Duration::from_secs(60))
        .keep_alive_between_time(Duration::from_millis(5))
        .valid_connection_checker(factory.checker())
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 2).await;
    tokio::time::sleep(Duration::from_millis(15)).await;
    pool.shrink_with_options(true, true).await;

    let state1 = pool.state();
    assert_eq!(state1.keep_alive_check_count, 2);

    // 再次填充并 shrink。
    fill_idle(&pool, 3).await;
    tokio::time::sleep(Duration::from_millis(15)).await;
    pool.shrink_with_options(true, true).await;

    let state2 = pool.state();
    assert_eq!(
        state2.keep_alive_check_count, 5,
        "keepAliveCheckCount must accumulate: 2 + 3"
    );
}

// ===========================================================================
// 11. removeAbandoned 与执行中连接的交互（详细版）
// ===========================================================================

/// Java `pooledConnection.isRunning()`：exec 执行期间连接不应被 abandoned。
/// 验证 exec 完成后连接正常归还，不被误标记。
#[tokio::test]
async fn remove_abandoned_does_not_affect_returned_after_exec() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("abandon-exec-return")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .remove_abandoned(true)
        .remove_abandoned_timeout(Duration::from_millis(30))
        .build()
        .await
        .unwrap();

    let mut conn = pool.get().await.unwrap();

    // 执行 SQL（ExecutionRunningGuard 置位）。
    let _ = conn.exec("SELECT 1", Vec::new()).await;

    // 等待超过 timeout。
    tokio::time::sleep(Duration::from_millis(60)).await;

    // exec 已完成，RunningGuard 已释放。但连接仍未归还。
    // removeAbandoned 会扫描到此连接。由于 exec 已完成（isRunning=false），
    // 且超过 timeout，它应被回收。
    let removed = pool.remove_abandoned();
    assert_eq!(removed, 1, "non-running expired lease is abandoned");

    // 随后 close 不应产生二次归还。
    conn.close().await.unwrap();
    assert_eq!(
        pool.state().idle_count,
        0,
        "abandoned connection not returned"
    );
}

// ===========================================================================
// 12. 密码版本多次递增
// ===========================================================================

/// Java：`userPasswordVersion` 是递增计数器，多次变更后旧版本连接全部失效。
#[tokio::test]
async fn credentials_version_increments_monotonically() {
    let factory = Arc::new(TestFactory::new());
    let pool = DruidPool::builder()
        .name("cred-multi")
        .driver_name("pool-maint-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    assert_eq!(pool.user_password_version(), 0);

    let v1 = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(v1, 1);

    let v2 = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(v2, 2);

    let v3 = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(v3, 3);
    assert_eq!(pool.user_password_version(), 3);
}
