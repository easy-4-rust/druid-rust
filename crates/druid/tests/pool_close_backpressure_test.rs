//! 物理关闭变慢时，池的关闭命令必须有硬容量上限。

use druid::core::{Connection, ConnectionFactory, ExecResult, RdbcError, Row, Value};
use druid::pool::DruidPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
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
