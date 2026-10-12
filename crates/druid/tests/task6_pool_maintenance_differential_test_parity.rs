//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Java Druid 池维护差分测试（C2 Step 33-35 验证批次）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 对照实现：
//! - `DruidDataSource#removeAbandoned`（`DestroyTask` 调用）+
//!   对照测试 `com.alibaba.druid.pvt.pool.TestAbondon`
//!   （removeAbandoned=true、timeout=10ms、logAbandoned=true、
//!   借出后超时连接被标记 abandoned）。
//! - `DruidDataSource#recycle` + `shrink` 凭据版本失效
//!   （`holder.userPasswordVersion < getUserPasswordVersion()` → discard）。
//! - `DruidAbstractDataSource#validateConnection` 的
//!   `if (result && onFatalError) { onFatalError = false; }` 恢复语义。

use druid::core::{
    Connection, ConnectionFactory, DruidError, ExecResult, MySqlExceptionSorter, RdbcError, Row,
    ValidConnectionChecker, Value,
};
use druid::pool::DruidPool;
use std::error::Error as _;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct MaintenanceConnection {
    closed_count: Arc<AtomicU64>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(MaintenanceConnection => Connection);

#[async_trait::async_trait]
impl Connection for MaintenanceConnection {
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
            self.closed_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("maintenance-diff".to_owned())
    }
}

struct DifferentialFactory {
    create_count: AtomicU64,
    validate_count: Arc<AtomicU64>,
    validation_succeeds: Arc<AtomicBool>,
    closed_count: Arc<AtomicU64>,
    create_gate: std::sync::Mutex<Option<Arc<tokio::sync::Semaphore>>>,
}

impl DifferentialFactory {
    fn new() -> Self {
        Self {
            create_count: AtomicU64::new(0),
            validate_count: Arc::new(AtomicU64::new(0)),
            validation_succeeds: Arc::new(AtomicBool::new(true)),
            closed_count: Arc::new(AtomicU64::new(0)),
            create_gate: std::sync::Mutex::new(None),
        }
    }

    /// 控制真实建连的顺序，隔离 fatal 状态断言与后台成功校验的竞态。
    fn set_create_gate(&self, gate: Arc<tokio::sync::Semaphore>) {
        *self.create_gate.lock().expect("create gate poisoned") = Some(gate);
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for DifferentialFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        let gate = self
            .create_gate
            .lock()
            .expect("create gate poisoned")
            .clone();
        if let Some(gate) = gate {
            gate.acquire()
                .await
                .expect("create gate remains open")
                .forget();
        }
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(MaintenanceConnection {
            closed_count: self.closed_count.clone(),
            closed: false,
        }))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        self.validate_count.fetch_add(1, Ordering::Relaxed);
        if self.validation_succeeds.load(Ordering::Relaxed) {
            connection.ping().await
        } else {
            Err(RdbcError::ValidationFailed(
                "differential validation failed".to_string(),
            ))
        }
    }
}

async fn fill_idle(pool: &DruidPool, count: usize) {
    let mut connections = Vec::with_capacity(count);
    for _ in 0..count {
        connections.push(pool.get().await.unwrap());
    }
    for mut connection in connections {
        connection.close().await.unwrap();
    }
}

// ── removeAbandoned（Java DruidDataSource#removeAbandoned / TestAbondon）──

// Java `TestAbondon#test_0` 语义：removeAbandoned=true 且超时后，
// 借出中的连接被扫描判定 abandoned 并计入 removeAbandonedCount。

// Java：`pooledConnection.isRunning()` 时 continue，不回收执行中的连接。
//
// `execution_running` 守卫由 `ExecutionRunningGuard`（RAII）在一次
// exec/fetch 执行期间持有；测试用阻塞中的 exec 并发触发扫描验证。

// Java：removeAbandoned=false 时直接返回 0（`isRemoveAbandoned()` 门）。

// ── 凭据版本动态失效（Java recycle/shrink/credentials 版本检查）────────

// Java `recycle`：`holder.userPasswordVersion < getUserPasswordVersion()`
// → discardConnection(holder)。旧版本借出连接归还时被丢弃。

// Java `shrink`（`CreateConnectionThread` 中的替换逻辑）：
// 空闲队列中旧版本连接被替换，随后按 targetTotal 回填新版本连接。

// Java：凭据未变更时归还连接正常回池（版本相等不丢弃）。

// ── onFatalError 统一流（Java validateConnection 恢复语义）─────────────

// Java `handleFatalError` 触发条件：SQL 异常 + sorter 判定 fatal。
// 默认 `onFatalErrorMaxActive=0` 时首个 fatal 即置位 onFatalError。
struct FatalSortingChecker {
    validation_succeeds: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl ValidConnectionChecker for FatalSortingChecker {
    async fn is_valid_connection(
        &self,
        _connection: &mut Box<dyn Connection>,
        _query: Option<&str>,
        _validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        Ok(self.validation_succeeds.load(Ordering::Relaxed))
    }
}

// Java `DruidAbstractDataSource#validateConnection`：
// `if (result && onFatalError) { onFatalError = false; }` — fatal 状态在
// 一次成功验证后被清除。经 `handle_exception` 注入 fatal、借走验证清除。

// Java：验证持续失败时 onFatalError 保持置位（不清除）。

// Java `onFatalErrorMaxActive` 门禁：increment 超过阈值才置位。

#[tokio::test]
async fn task6_pool_maintenance_differential_test_remove_abandoned_reclaims_timed_out_borrowed_connection(
) {
    let factory = Arc::new(DifferentialFactory::new());
    let pool = DruidPool::builder()
        .name("abandon-test")
        .driver_name("maintenance-diff")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .remove_abandoned(true)
        .remove_abandoned_timeout(Duration::from_millis(10))
        .log_abandoned(true)
        .build()
        .await
        .unwrap();

    // 借出且不归还（模拟泄漏）。
    let mut leaked = pool.get().await.unwrap();
    let leaked_id = leaked.id();
    tokio::time::sleep(Duration::from_millis(30)).await;

    let removed = pool.remove_abandoned();

    // Java：timeMillis >= removeAbandonedTimeoutMillis → iter.remove + abandond()。
    assert_eq!(removed, 1, "one timed-out lease should be reclaimed");
    let state = pool.state();
    assert_eq!(state.leak_detection_count, 1, "removeAbandonedCount++");

    // 借出句柄随后关闭：租约已失效，静默完成不产生二次归还计数。
    leaked.close().await.unwrap();

    // 未超时的借出连接不受影响。
    let fresh = pool.get().await.unwrap();
    let removed_again = pool.remove_abandoned();
    assert_eq!(removed_again, 0, "fresh borrow within timeout must survive");
    drop(fresh);
    assert_ne!(leaked_id, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task6_pool_maintenance_differential_test_remove_abandoned_skips_running_execution() {
    struct BlockingConnection {
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    druid_rdbc::impl_standard_wrapper!(BlockingConnection => Connection);

    #[async_trait::async_trait]
    impl Connection for BlockingConnection {
        async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
            let release = self.release.notified();
            tokio::pin!(release);
            release.as_mut().enable();
            self.entered.notify_one();
            release.await;
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
            Ok(())
        }
        fn driver_name(&self) -> Result<String, RdbcError> {
            Ok("blocking".to_owned())
        }
        fn is_closed(&self) -> Result<bool, RdbcError> {
            Ok(false)
        }
    }
    struct BlockingFactory {
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    #[async_trait::async_trait]
    impl ConnectionFactory for BlockingFactory {
        async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
            Ok(Box::new(BlockingConnection {
                entered: self.entered.clone(),
                release: self.release.clone(),
            }))
        }
        async fn validate(&self, _connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
            Ok(())
        }
    }

    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let pool = DruidPool::builder()
        .name("abandon-running")
        .driver_name("blocking")
        .factory(Arc::new(BlockingFactory {
            entered: entered.clone(),
            release: release.clone(),
        }))
        .max_open(4)
        .max_idle(4)
        .remove_abandoned(true)
        .remove_abandoned_timeout(Duration::from_millis(10))
        .acquire_timeout(Duration::from_secs(1))
        .build()
        .await
        .unwrap();
    let pool = Arc::new(pool);

    let mut connection = pool.get().await.unwrap();
    let execution = tokio::spawn(async move {
        // exec 期间 ExecutionRunningGuard 持有 execution_running=true。
        let _ = connection.exec("SELECT 1", Vec::new()).await;
        let _ = connection.close().await;
    });
    // 先确认 exec 已注册 release waiter，再越过 removeAbandonedTimeout。
    if tokio::time::timeout(Duration::from_secs(1), entered.notified())
        .await
        .is_err()
    {
        release.notify_one();
        let _ = execution.await;
        panic!("canonical pooled exec did not reach the running driver operation");
    }
    tokio::time::sleep(Duration::from_millis(50)).await;

    let safety_release = release.clone();
    let (cancel_safety, safety_cancelled) = std::sync::mpsc::channel();
    let safety = std::thread::spawn(move || {
        if safety_cancelled
            .recv_timeout(Duration::from_secs(1))
            .is_err()
        {
            safety_release.notify_one();
        }
    });
    let scan_started = std::time::Instant::now();
    let removed = pool.remove_abandoned();
    let scan_elapsed = scan_started.elapsed();
    let _ = cancel_safety.send(());
    let _ = safety.join();
    assert!(
        scan_elapsed < Duration::from_secs(1),
        "removeAbandoned must not block on a running execution: {scan_elapsed:?}"
    );
    assert_eq!(removed, 0, "running connection must not be abandoned");

    // 释放阻塞执行并等待归还完成。
    release.notify_one();
    let _ = execution.await;
    let state = pool.state();
    assert_eq!(state.active_count, 0, "execution completes and recycles");
}

#[tokio::test]
async fn task6_pool_maintenance_differential_test_remove_abandoned_disabled_returns_zero() {
    let factory = Arc::new(DifferentialFactory::new());
    let pool = DruidPool::builder()
        .name("abandon-disabled")
        .driver_name("maintenance-diff")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let _leaked = pool.get().await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(pool.remove_abandoned(), 0);
}

#[tokio::test]
async fn task6_pool_maintenance_differential_test_credentials_change_discards_stale_connection_on_recycle(
) {
    let factory = Arc::new(DifferentialFactory::new());
    let pool = DruidPool::builder()
        .name("credentials-recycle")
        .driver_name("maintenance-diff")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 2).await;

    let mut stale_version_conn = pool.get().await.unwrap();
    let version_before = pool.user_password_version();
    let version_after = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(version_after, version_before + 1);

    // 旧版本连接归还 → discard（不回池）。
    stale_version_conn.close().await.unwrap();
    let state = pool.state();
    assert!(
        state.discard_count >= 1,
        "stale-version recycle must discard, state={state:?}"
    );
}

#[tokio::test]
async fn task6_pool_maintenance_differential_test_credentials_change_replaces_idle_and_refills() {
    let factory = Arc::new(DifferentialFactory::new());
    let pool = DruidPool::builder()
        .name("credentials-idle")
        .driver_name("maintenance-diff")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 3).await;
    let creates_before = factory.create_count.load(Ordering::Relaxed);

    let version = pool.notify_credentials_changed().await.unwrap();
    assert_eq!(version, 1);

    // 旧空闲被销毁，新连接按原 total 回填。
    let creates_after = factory.create_count.load(Ordering::Relaxed);
    assert!(
        creates_after >= creates_before + 3,
        "idle replacement must recreate pool size: before={creates_before} after={creates_after}"
    );
    let state = pool.state();
    assert_eq!(state.idle_count, 3, "refill must restore idle count");
}

#[tokio::test]
async fn task6_pool_maintenance_differential_test_unchanged_credentials_recycle_normally() {
    let factory = Arc::new(DifferentialFactory::new());
    let pool = DruidPool::builder()
        .name("credentials-same")
        .driver_name("maintenance-diff")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .build()
        .await
        .unwrap();

    let mut connection = pool.get().await.unwrap();
    connection.close().await.unwrap();
    let state = pool.state();
    assert_eq!(state.idle_count, 1);
    assert_eq!(state.discard_count, 0);
}

#[tokio::test]
async fn task6_pool_maintenance_differential_test_successful_validation_clears_on_fatal_error() {
    let factory = Arc::new(DifferentialFactory::new());
    let validation_succeeds = Arc::new(AtomicBool::new(true));
    let checker: Arc<dyn ValidConnectionChecker> = Arc::new(FatalSortingChecker {
        validation_succeeds: Arc::clone(&validation_succeeds),
    });

    let pool = DruidPool::builder()
        .name("fatal-clear")
        .driver_name("maintenance-diff")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .test_on_borrow(true)
        .valid_connection_checker(checker)
        .exception_sorter(Arc::new(MySqlExceptionSorter))
        .build()
        .await
        .unwrap();

    // 借出连接并注入 fatal SQL 异常（MySql sorter 判 1042 为 fatal）。
    let mut connection = pool.get().await.unwrap();
    assert!(!pool.is_on_fatal_error());
    let fatal = RdbcError::SqlException(Box::new(druid::core::SqlException::driver(
        1042,
        "fatal connection error".to_owned(),
    )));
    let discarded = connection.handle_rdbc_exception(&fatal);
    assert!(
        discarded,
        "fatal-sorted exception must discard the connection"
    );
    assert!(
        pool.is_on_fatal_error(),
        "default onFatalErrorMaxActive=0 sets onFatalError on first fatal"
    );
    // 注入后连接已 discard；close 走销毁路径。
    let _ = connection.close().await;

    // 恢复验证成功并借出：borrow 的 testOnBorrow 验证成功清除 onFatalError。
    validation_succeeds.store(true, Ordering::Relaxed);
    let mut recovered = pool.get().await.unwrap();
    assert!(
        !pool.is_on_fatal_error(),
        "successful validation must clear onFatalError (Java validateConnection)"
    );
    let _ = recovered.close().await;
}

#[tokio::test]
async fn task6_pool_maintenance_differential_test_failing_validation_keeps_on_fatal_error() {
    let factory = Arc::new(DifferentialFactory::new());
    let create_gate = Arc::new(tokio::sync::Semaphore::new(1));
    factory.set_create_gate(Arc::clone(&create_gate));
    let validation_succeeds = Arc::new(AtomicBool::new(true));
    let checker: Arc<dyn ValidConnectionChecker> = Arc::new(FatalSortingChecker {
        validation_succeeds: Arc::clone(&validation_succeeds),
    });

    let pool = DruidPool::builder()
        .name("fatal-keep")
        .driver_name("maintenance-diff")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .test_on_borrow(true)
        .valid_connection_checker(checker)
        .exception_sorter(Arc::new(MySqlExceptionSorter))
        .acquire_timeout(Duration::from_millis(300))
        .build()
        .await
        .unwrap();

    // 初始验证成功：正常借出。
    let mut connection = pool.get_timeout(Duration::from_millis(200)).await.unwrap();
    let fatal = RdbcError::SqlException(Box::new(druid::core::SqlException::driver(
        1042,
        "fatal connection error".to_owned(),
    )));
    assert!(connection.handle_rdbc_exception(&fatal));
    let _ = connection.close().await;
    assert!(pool.is_on_fatal_error());
    // fatal 置位后再翻验证为失败。
    validation_succeeds.store(false, Ordering::Relaxed);
    // Fatal 会触发真实后台补池；先保留原置位观察，再允许失败校验发生。
    create_gate.add_permits(1);

    // 验证失败期间再借出：超时返回（不 crash），onFatalError 保持。
    let second = pool.get_timeout(Duration::from_millis(100)).await;
    assert!(
        second.is_err(),
        "borrow with failing validation must time out"
    );
    assert!(
        pool.is_on_fatal_error(),
        "failing validation must keep onFatalError"
    );
    assert_eq!(factory.create_count.load(Ordering::Relaxed), 2);
    assert_eq!(factory.closed_count.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn task6_pool_maintenance_differential_test_on_fatal_error_max_active_gates_onset() {
    let factory = Arc::new(DifferentialFactory::new());
    let create_gate = Arc::new(tokio::sync::Semaphore::new(3));
    factory.set_create_gate(Arc::clone(&create_gate));
    let pool = DruidPool::builder()
        .name("fatal-max-active")
        .driver_name("maintenance-diff")
        .factory(factory)
        .max_open(8)
        .max_idle(8)
        .on_fatal_error_max_active(2)
        .exception_sorter(Arc::new(MySqlExceptionSorter))
        .build()
        .await
        .unwrap();

    assert_eq!(pool.on_fatal_error_max_active(), 2);

    // 第 1、2 个 fatal：increment(1,2) <= 2 → 不置位。
    for _ in 0..2 {
        let mut connection = pool.get().await.unwrap();
        let fatal = RdbcError::SqlException(Box::new(druid::core::SqlException::driver(
            1042,
            "fatal connection error".to_owned(),
        )));
        assert!(connection.handle_rdbc_exception(&fatal));
        let _ = connection.close().await;
    }
    assert!(!pool.is_on_fatal_error(), "threshold not exceeded yet");

    // 第 3 个 fatal：increment(3) > 2 → 置位。
    let mut connection = pool.get().await.unwrap();
    let fatal = RdbcError::SqlException(Box::new(druid::core::SqlException::driver(
        1042,
        "fatal connection error".to_owned(),
    )));
    assert!(connection.handle_rdbc_exception(&fatal));
    let _ = connection.close().await;
    assert!(
        pool.is_on_fatal_error(),
        "increment above onFatalErrorMaxActive sets onFatalError"
    );
    // 原第三次置位断言后才开放补池；成功验证应按 Java 合同清除 fatal。
    // 后台 creator 与直接借用可各占一次建连，两个路径均有独立容量预算。
    create_gate.add_permits(2);
    let mut recovered = pool.get_timeout(Duration::from_secs(1)).await.unwrap();
    assert!(
        !pool.is_on_fatal_error(),
        "successful refill validation clears fatal"
    );
    recovered.close().await.unwrap();
}

#[tokio::test]
async fn task6_pool_maintenance_differential_test_pool_trait_get_preserves_fatal_product_class_and_typed_source(
) {
    let factory = Arc::new(DifferentialFactory::new());
    let pool = DruidPool::builder()
        .name("fatal-product-boundary")
        .driver_name("maintenance-diff")
        .factory(factory)
        .max_open(8)
        .max_idle(8)
        .on_fatal_error_max_active(2)
        .exception_sorter(Arc::new(MySqlExceptionSorter))
        .build()
        .await
        .expect("fatal-boundary pool must build");

    for _ in 0..2 {
        let mut connection = pool.get().await.expect("pre-threshold borrow must succeed");
        let fatal = RdbcError::SqlException(Box::new(druid::core::SqlException::driver(
            1042,
            "fatal connection error".to_owned(),
        )));
        assert!(connection.handle_rdbc_exception(&fatal));
        let _ = connection.close().await;
    }

    let mut held_one = pool.get().await.expect("first held borrow must succeed");
    let mut held_two = pool.get().await.expect("second held borrow must succeed");
    let mut trigger = pool.get().await.expect("fatal trigger borrow must succeed");
    let fatal = RdbcError::SqlException(Box::new(druid::core::SqlException::driver(
        1042,
        "fatal connection error".to_owned(),
    )));
    assert!(trigger.handle_rdbc_exception(&fatal));
    let _ = trigger.close().await;
    assert!(pool.is_on_fatal_error());

    let error = <DruidPool as druid::core::Pool>::get(&pool)
        .await
        .expect_err("Pool trait boundary must reject at the fatal active threshold");
    assert_eq!(error.class_name(), "java.sql.SQLException");
    assert!(error.to_string().contains("onFatalError"));
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<RdbcError>())
        .expect("product boundary must retain a typed RDBC source");
    assert_eq!(source.class_name(), "java.sql.SQLException");
    assert_eq!(
        source
            .sql_exception()
            .map(druid::core::SqlException::error_code),
        Some(1042)
    );

    let _ = held_one.close().await;
    let _ = held_two.close().await;
}
