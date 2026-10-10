//! 物理关闭变慢时，池的关闭命令必须有硬容量上限。

use druid::core::{
    BeforeFilter, Connection, ConnectionFactory, DruidError, ExecContext, ExecResult, FilterChain,
    RdbcError, Row, ValidConnectionChecker, Value,
};
use druid::pool::{DataSourceStatSink, DruidPool};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

/// 可阻塞物理关闭的测试连接。对应 Java: DruidDataSource 的物理 Connection。
struct SlowCloseConnection {
    gate: Arc<Semaphore>,
    entered: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
}

druid_rdbc::impl_standard_wrapper!(SlowCloseConnection => Connection);

#[async_trait::async_trait]
impl Connection for SlowCloseConnection {
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
        self.entered.fetch_add(1, Ordering::AcqRel);
        let permit = self.gate.acquire().await.expect("close gate remains open");
        permit.forget();
        self.closed.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

/// 为每条新连接共享同一个慢关闭闸门。对应 Java: DruidDataSource 的连接工厂。
struct SlowCloseFactory {
    gate: Arc<Semaphore>,
    entered: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
}

/// 首条物理关闭阻塞，后续连接关闭立即完成，以区分队列溢出与等待 worker。
struct SelectiveSlowCloseFactory {
    first_gate: Arc<Semaphore>,
    block_overflow: bool,
    created: AtomicUsize,
    entered: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
}

/// 首次建连等待外部闸门，用于复现关闭时仍在运行的借用任务。
struct HangingCreateFactory {
    gate: Arc<Semaphore>,
    entered: Arc<AtomicUsize>,
    created: AtomicUsize,
}

/// 销毁等待闸门的过滤器，验证关闭超时后可从同一生命周期继续等待。
struct HangingDestroyFilter {
    gate: Arc<Semaphore>,
    entered: Arc<AtomicUsize>,
    completed: Arc<AtomicUsize>,
}

/// 首轮同步发布受控阻塞，模拟统计 sink 卡住 Tokio worker 的场景。
struct BlockingStatSink {
    entered: Arc<AtomicUsize>,
    release: Mutex<std_mpsc::Receiver<()>>,
}

/// 第二次校验阻塞，首次校验用于初始建连，第二次由维护任务保活触发。
struct BlockingValidationChecker {
    gate: Arc<Semaphore>,
    entered: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl ValidConnectionChecker for BlockingValidationChecker {
    async fn is_valid_connection(
        &self,
        _connection: &mut Box<dyn Connection>,
        _query: Option<&str>,
        _validation_query_timeout: Duration,
    ) -> Result<bool, DruidError> {
        if self.entered.fetch_add(1, Ordering::AcqRel) == 1 {
            let permit = self
                .gate
                .acquire()
                .await
                .expect("validation gate remains open");
            permit.forget();
        }
        Ok(true)
    }
}

impl DataSourceStatSink for BlockingStatSink {
    fn publish(
        &self,
        _stat_value: &druid::stats::DruidDataSourceStatValue,
    ) -> Result<(), DruidError> {
        if self.entered.fetch_add(1, Ordering::AcqRel) == 0 {
            self.release
                .lock()
                .expect("sink lock")
                .recv()
                .expect("release sink");
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl BeforeFilter for HangingDestroyFilter {
    fn name(&self) -> &str {
        "hanging-destroy"
    }

    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }

    async fn destroy(&self) -> Result<(), DruidError> {
        self.entered.fetch_add(1, Ordering::AcqRel);
        let permit = self.gate.acquire().await.expect("filter gate remains open");
        permit.forget();
        self.completed.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for HangingCreateFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        if self.created.fetch_add(1, Ordering::AcqRel) == 0 {
            self.entered.fetch_add(1, Ordering::AcqRel);
            let permit = self.gate.acquire().await.expect("create gate remains open");
            permit.forget();
        }
        Ok(Box::new(SlowCloseConnection {
            gate: Arc::new(Semaphore::new(1)),
            entered: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicUsize::new(0)),
        }))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for SelectiveSlowCloseFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        let index = self.created.fetch_add(1, Ordering::AcqRel);
        let gate = if index == 0 || (index >= 2 && self.block_overflow) {
            Arc::clone(&self.first_gate)
        } else {
            Arc::new(Semaphore::new(1))
        };
        Ok(Box::new(SlowCloseConnection {
            gate,
            entered: Arc::clone(&self.entered),
            closed: Arc::clone(&self.closed),
        }))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for SlowCloseFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(SlowCloseConnection {
            gate: Arc::clone(&self.gate),
            entered: Arc::clone(&self.entered),
            closed: Arc::clone(&self.closed),
        }))
    }

    async fn validate(&self, connection: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        connection.ping().await
    }
}

#[tokio::test]
async fn slow_physical_close_has_bounded_queue_and_shutdown_joins_worker() {
    const CAPACITY: usize = 4;
    const ROUNDS: usize = 24;
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SlowCloseFactory {
        gate: Arc::clone(&gate),
        entered: Arc::clone(&entered),
        closed: Arc::clone(&closed),
    });
    let pool = DruidPool::builder()
        .name("slow-close-backpressure")
        .driver_name("test")
        .factory(factory)
        .max_open(2)
        .max_idle(0)
        .close_queue_capacity(CAPACITY)
        .build()
        .await
        .expect("build pool");

    // 持续销毁超过 max_open + 队列容量的连接，验证生产者不会无限积压。
    tokio::time::timeout(Duration::from_secs(5), async {
        for round in 0..ROUNDS {
            let mut connection = tokio::time::timeout(Duration::from_secs(1), pool.get())
                .await
                .unwrap_or_else(|_| panic!("borrow stalled at round {round}: {:?}", pool.state()))
                .expect("borrow connection");
            connection.discard_connection();
        }
    })
    .await
    .expect("close storm remains bounded");

    let state = pool.state();
    assert!(state.close_queue_depth <= CAPACITY);
    assert!(state.close_queue_rejected_total > 0);
    tokio::time::timeout(Duration::from_secs(1), async {
        while entered.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("close worker starts processing");

    gate.add_permits(ROUNDS + CAPACITY);
    tokio::time::timeout(Duration::from_secs(5), pool.close())
        .await
        .expect("shutdown joins close worker");
    assert_eq!(pool.state().close_queue_depth, 0);
    assert!(closed.load(Ordering::Acquire) <= ROUNDS);
}

#[tokio::test]
async fn slow_physical_close_obeys_configured_deadline() {
    let gate = Arc::new(Semaphore::new(0));
    let factory = Arc::new(SlowCloseFactory {
        gate,
        entered: Arc::new(AtomicUsize::new(0)),
        closed: Arc::new(AtomicUsize::new(0)),
    });
    let pool = DruidPool::builder()
        .name("slow-close-timeout")
        .driver_name("test")
        .factory(factory)
        .max_open(1)
        .close_timeout(Duration::from_millis(25))
        .build()
        .await
        .expect("build pool");

    let mut connection = pool.get().await.expect("borrow connection");
    connection.discard_connection();
    tokio::time::timeout(Duration::from_secs(2), pool.close())
        .await
        .expect("slow physical close does not hold shutdown forever");
    assert_eq!(pool.state().close_timeout_total, 1);
    assert!(pool.state().close_worker_terminated);
}

#[tokio::test]
async fn shutdown_deadline_aborts_stalled_close_worker() {
    let entered = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SlowCloseFactory {
        gate: Arc::new(Semaphore::new(0)),
        entered: Arc::clone(&entered),
        closed: Arc::new(AtomicUsize::new(0)),
    });
    let pool = DruidPool::builder()
        .name("stalled-close-shutdown")
        .driver_name("test")
        .factory(factory)
        .max_open(1)
        .close_timeout(Duration::from_secs(2))
        .shutdown_timeout(Duration::from_millis(50))
        .build()
        .await
        .expect("build pool");
    let mut connection = pool.get().await.expect("borrow connection");
    connection.discard_connection();
    tokio::time::timeout(Duration::from_secs(1), async {
        while entered.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("close worker enters physical close");

    tokio::time::timeout(Duration::from_millis(500), pool.close())
        .await
        .expect("shutdown deadline bounds stalled worker");
    assert_eq!(pool.state().close_queue_depth, 0);
    assert!(!pool.state().close_worker_terminated);
}

#[tokio::test]
async fn explicit_close_attempts_physical_close_when_close_queue_is_full() {
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SelectiveSlowCloseFactory {
        first_gate: Arc::clone(&gate),
        block_overflow: false,
        created: AtomicUsize::new(0),
        entered: Arc::clone(&entered),
        closed: Arc::clone(&closed),
    });
    let pool = DruidPool::builder()
        .name("explicit-close-overflow")
        .driver_name("test")
        .factory(factory)
        .max_open(1)
        .max_use_count(1)
        .close_queue_capacity(1)
        .close_timeout(Duration::from_secs(5))
        .build()
        .await
        .expect("build pool");

    let mut first = pool.get().await.expect("borrow first connection");
    first.discard_connection();
    tokio::time::timeout(Duration::from_secs(1), async {
        while entered.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("close worker enters first physical close");

    let mut second = pool.get().await.expect("borrow second connection");
    second.discard_connection();
    assert_eq!(pool.state().close_queue_depth, 1);

    let mut third = pool.get().await.expect("borrow third connection");
    tokio::time::timeout(Duration::from_secs(2), third.close())
        .await
        .expect("explicit close remains deadline bounded")
        .expect("explicit close completes the overflow holder");
    assert_eq!(entered.load(Ordering::Acquire), 2);
    assert_eq!(closed.load(Ordering::Acquire), 1);
    assert!(pool.state().close_queue_rejected_total > 0);
    gate.add_permits(1);
    tokio::time::timeout(Duration::from_secs(2), pool.close())
        .await
        .expect("pool closes after overflow");
}

#[tokio::test]
async fn explicit_overflow_close_reports_physical_timeout() {
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(SelectiveSlowCloseFactory {
        first_gate: Arc::clone(&gate),
        block_overflow: true,
        created: AtomicUsize::new(0),
        entered: Arc::clone(&entered),
        closed: Arc::new(AtomicUsize::new(0)),
    });
    let pool = DruidPool::builder()
        .name("explicit-close-overflow-timeout")
        .driver_name("test")
        .factory(factory)
        .max_open(1)
        .max_use_count(1)
        .close_queue_capacity(1)
        .close_timeout(Duration::from_millis(250))
        .build()
        .await
        .expect("build pool");

    let mut first = pool.get().await.expect("borrow first connection");
    first.discard_connection();
    tokio::time::timeout(Duration::from_secs(1), async {
        while entered.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first physical close begins");
    let mut second = pool.get().await.expect("borrow second connection");
    second.discard_connection();
    assert_eq!(pool.state().close_queue_depth, 1);

    let mut third = pool.get().await.expect("borrow third connection");
    let error = tokio::time::timeout(Duration::from_secs(2), third.close())
        .await
        .expect("explicit close is bounded")
        .expect_err("timed out physical close must be reported");
    assert!(error
        .to_string()
        .contains("physical connection close timed out"));
    assert_eq!(entered.load(Ordering::Acquire), 2);
    assert!(pool.state().close_queue_rejected_total > 0);
    assert!(pool.state().close_timeout_total > 0);

    gate.add_permits(1);
    tokio::time::timeout(Duration::from_secs(2), pool.close())
        .await
        .expect("shutdown remains bounded");
}

#[tokio::test]
async fn shutdown_deadline_defers_cleanup_until_external_creator_finishes() {
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(AtomicUsize::new(0));
    let factory = Arc::new(HangingCreateFactory {
        gate: Arc::clone(&gate),
        entered: Arc::clone(&entered),
        created: AtomicUsize::new(0),
    });
    let pool = Arc::new(
        DruidPool::builder()
            .name("stalled-creator-shutdown")
            .driver_name("test")
            .factory(factory)
            .max_open(1)
            .shutdown_timeout(Duration::from_millis(50))
            .build()
            .await
            .expect("build pool"),
    );
    let borrowing_pool = Arc::clone(&pool);
    let borrow = tokio::spawn(async move { borrowing_pool.get().await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while entered.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("factory begins creating");

    tokio::time::timeout(Duration::from_millis(500), pool.close())
        .await
        .expect("shutdown has a bounded creator wait");
    assert!(pool.is_closed());
    assert!(
        tokio::time::timeout(Duration::from_millis(500), pool.restart())
            .await
            .expect("restart fails promptly while creator remains")
            .is_err()
    );

    gate.add_permits(1);
    assert!(tokio::time::timeout(Duration::from_secs(2), borrow)
        .await
        .expect("old borrow terminates")
        .expect("old borrow task joins")
        .is_err());
    tokio::time::timeout(Duration::from_secs(2), pool.restart())
        .await
        .expect("restart completes after creator drains")
        .expect("restart is safe after creator drains");
    let mut connection = pool.get().await.expect("new generation borrows");
    connection.close().await.expect("new lease closes");
    pool.close().await;
}

#[tokio::test]
async fn shutdown_deadline_retains_filter_destroy_until_completion() {
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicUsize::new(0));
    let mut chain = FilterChain::new();
    chain.add_before(Arc::new(HangingDestroyFilter {
        gate: Arc::clone(&gate),
        entered: Arc::clone(&entered),
        completed: Arc::clone(&completed),
    }));
    let pool = DruidPool::builder()
        .name("stalled-filter-destroy")
        .driver_name("test")
        .factory(Arc::new(SelectiveSlowCloseFactory {
            first_gate: Arc::new(Semaphore::new(1)),
            block_overflow: false,
            created: AtomicUsize::new(0),
            entered: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicUsize::new(0)),
        }))
        .filter_chain(Arc::new(chain))
        .shutdown_timeout(Duration::from_millis(50))
        .build()
        .await
        .expect("build pool");

    tokio::time::timeout(Duration::from_millis(500), pool.close())
        .await
        .expect("filter destroy cannot hold shutdown forever");
    assert_eq!(entered.load(Ordering::Acquire), 1);
    assert_eq!(completed.load(Ordering::Acquire), 0);
    assert!(
        tokio::time::timeout(Duration::from_millis(500), pool.restart())
            .await
            .expect("restart fails promptly while filter destroy runs")
            .is_err()
    );

    gate.add_permits(1);
    tokio::time::timeout(Duration::from_secs(1), async {
        while completed.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("filter destroy completes");
    tokio::time::timeout(Duration::from_secs(1), pool.restart())
        .await
        .expect("restart finishes after filter destroy")
        .expect("restart succeeds after filter destroy");
    assert_eq!(entered.load(Ordering::Acquire), 1);
    pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shutdown_deadline_retains_blocked_stat_publisher_until_completion() {
    let (release, receiver) = std_mpsc::channel();
    let entered = Arc::new(AtomicUsize::new(0));
    let pool = DruidPool::builder()
        .name("stalled-stat-publisher")
        .driver_name("test")
        .factory(Arc::new(SelectiveSlowCloseFactory {
            first_gate: Arc::new(Semaphore::new(1)),
            block_overflow: false,
            created: AtomicUsize::new(0),
            entered: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicUsize::new(0)),
        }))
        .stat_publish_interval(Duration::from_secs(3600))
        .stat_sink(Arc::new(BlockingStatSink {
            entered: Arc::clone(&entered),
            release: Mutex::new(receiver),
        }))
        .shutdown_timeout(Duration::from_millis(50))
        .build()
        .await
        .expect("build pool");
    pool.init().await.expect("start statistic publisher");
    tokio::time::timeout(Duration::from_secs(1), async {
        while entered.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("publisher enters sink");

    let close_result = tokio::time::timeout(Duration::from_millis(500), pool.close()).await;
    if close_result.is_err() {
        release
            .send(())
            .expect("release blocked sink after RED failure");
    }
    close_result.expect("blocked sink cannot hold shutdown forever");
    assert!(
        tokio::time::timeout(Duration::from_millis(500), pool.restart())
            .await
            .expect("restart fails promptly while prior sink runs")
            .is_err()
    );
    release.send(()).expect("release blocked sink");
    tokio::time::timeout(Duration::from_secs(1), pool.restart())
        .await
        .expect("restart completes after sink release")
        .expect("restart succeeds after sink release");
    pool.close().await;
}

#[tokio::test]
async fn shutdown_deadline_retains_blocked_maintenance_until_completion() {
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(AtomicUsize::new(0));
    let pool = DruidPool::builder()
        .name("stalled-maintenance")
        .driver_name("test")
        .factory(Arc::new(SelectiveSlowCloseFactory {
            first_gate: Arc::new(Semaphore::new(1)),
            block_overflow: false,
            created: AtomicUsize::new(0),
            entered: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicUsize::new(0)),
        }))
        .valid_connection_checker(Arc::new(BlockingValidationChecker {
            gate: Arc::clone(&gate),
            entered: Arc::clone(&entered),
        }))
        .initial_size(1)
        .min_idle(1)
        .keep_alive(true)
        .keep_alive_between_time(Duration::from_millis(10))
        .time_between_eviction_runs(Duration::from_millis(5))
        .shutdown_timeout(Duration::from_millis(50))
        .build()
        .await
        .expect("build pool");
    pool.init().await.expect("start maintenance");
    tokio::time::timeout(Duration::from_secs(2), async {
        while entered.load(Ordering::Acquire) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("maintenance enters keep-alive validation");

    let close_result = tokio::time::timeout(Duration::from_millis(500), pool.close()).await;
    if close_result.is_err() {
        gate.add_permits(1);
    }
    close_result.expect("blocked maintenance cannot hold shutdown forever");
    assert!(
        tokio::time::timeout(Duration::from_millis(500), pool.restart())
            .await
            .expect("restart fails promptly while maintenance runs")
            .is_err()
    );
    gate.add_permits(1);
    tokio::time::timeout(Duration::from_secs(1), pool.restart())
        .await
        .expect("restart completes after maintenance release")
        .expect("restart succeeds after maintenance release");
    pool.close().await;
}

#[tokio::test]
async fn synchronous_stat_sink_does_not_block_the_async_runtime() {
    let (release, receiver) = std_mpsc::channel();
    let entered = Arc::new(AtomicUsize::new(0));
    let pool = DruidPool::builder()
        .name("stat-sink-runtime-isolation")
        .driver_name("test")
        .factory(Arc::new(SelectiveSlowCloseFactory {
            first_gate: Arc::new(Semaphore::new(1)),
            block_overflow: false,
            created: AtomicUsize::new(0),
            entered: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicUsize::new(0)),
        }))
        .stat_publish_interval(Duration::from_secs(3600))
        .stat_sink(Arc::new(BlockingStatSink {
            entered: Arc::clone(&entered),
            release: Mutex::new(receiver),
        }))
        .build()
        .await
        .expect("build pool");
    let release_after = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        release.send(()).expect("release sink");
    });
    let started = Instant::now();
    pool.init().await.expect("start publisher");
    tokio::time::sleep(Duration::from_millis(30)).await;
    let elapsed = started.elapsed();
    release_after.join().expect("release thread joins");
    assert!(entered.load(Ordering::Acquire) > 0);
    assert!(
        elapsed < Duration::from_millis(200),
        "blocked sink stalled the async runtime for {elapsed:?}"
    );
    pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn shutdown_uses_one_deadline_across_publisher_and_filter_cleanup() {
    let (release_stat, receiver) = std_mpsc::channel();
    let stat_entered = Arc::new(AtomicUsize::new(0));
    let filter_gate = Arc::new(Semaphore::new(0));
    let filter_entered = Arc::new(AtomicUsize::new(0));
    let filter_completed = Arc::new(AtomicUsize::new(0));
    let mut chain = FilterChain::new();
    chain.add_before(Arc::new(HangingDestroyFilter {
        gate: Arc::clone(&filter_gate),
        entered: Arc::clone(&filter_entered),
        completed: Arc::clone(&filter_completed),
    }));
    let pool = DruidPool::builder()
        .name("one-shutdown-deadline")
        .driver_name("test")
        .factory(Arc::new(SelectiveSlowCloseFactory {
            first_gate: Arc::new(Semaphore::new(1)),
            block_overflow: false,
            created: AtomicUsize::new(0),
            entered: Arc::new(AtomicUsize::new(0)),
            closed: Arc::new(AtomicUsize::new(0)),
        }))
        .filter_chain(Arc::new(chain))
        .stat_publish_interval(Duration::from_secs(3600))
        .stat_sink(Arc::new(BlockingStatSink {
            entered: Arc::clone(&stat_entered),
            release: Mutex::new(receiver),
        }))
        .shutdown_timeout(Duration::from_millis(60))
        .build()
        .await
        .expect("build pool");
    pool.init().await.expect("start publisher");
    tokio::time::timeout(Duration::from_secs(1), async {
        while stat_entered.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("publisher enters sink");

    let stat_release = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(45)).await;
        release_stat.send(()).expect("release stat sink");
    });
    let filter_release = tokio::spawn({
        let filter_gate = Arc::clone(&filter_gate);
        let filter_entered = Arc::clone(&filter_entered);
        async move {
            while filter_entered.load(Ordering::Acquire) == 0 {
                tokio::task::yield_now().await;
            }
            tokio::time::sleep(Duration::from_millis(70)).await;
            filter_gate.add_permits(1);
        }
    });
    let started = Instant::now();
    pool.close().await;
    let elapsed = started.elapsed();
    let completed_at_return = filter_completed.load(Ordering::Acquire);
    stat_release.await.expect("stat release joins");
    filter_release.await.expect("filter release joins");
    assert!(
        elapsed < Duration::from_millis(100),
        "shutdown multiplied its 60 ms budget: {elapsed:?}"
    );
    assert_eq!(completed_at_return, 0);
    tokio::time::timeout(Duration::from_secs(1), pool.restart())
        .await
        .expect("restart after background cleanup completes")
        .expect("restart succeeds after background cleanup");
    pool.close().await;
}
