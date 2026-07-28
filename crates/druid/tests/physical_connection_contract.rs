//! `DruidPooledConnection -> PhysicalConnection` 垂直切片合同测试。
//!
//! 语义来源：
//! - `DruidPooledConnection#close/recycle`
//! - `DruidDataSource#getConnectionInternal/recycle`
//! - `FilterChainImpl` 的前向进入、逆向退出调用链

use druid::core::{
    AfterFilter, BeforeFilter, ConnectionFactory, DruidError, ExecContext, ExecResult, FilterChain,
    PhysicalConnection, Row, Value,
};
use druid::pool::DruidPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, Semaphore};

struct ContractPhysicalConnection;

#[async_trait::async_trait]
impl PhysicalConnection for ContractPhysicalConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: Some(7),
            row_count: None,
        })
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![
            Row::new(vec![Value::Int(1)]),
            Row::new(vec![Value::Int(2)]),
        ])
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
        Ok(())
    }

    fn driver_name(&self) -> &str {
        "contract"
    }
}

struct ContractFactory;

#[async_trait::async_trait]
impl ConnectionFactory for ContractFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Ok(Box::new(ContractPhysicalConnection))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}

#[derive(Clone)]
struct BeforeSnapshot {
    sql: String,
    params: Vec<Value>,
    data_source: String,
    start: Instant,
}

struct ContextContractFilter {
    before: Mutex<Option<BeforeSnapshot>>,
    after_count: AtomicUsize,
    row_counts: Mutex<Vec<Option<u64>>>,
}

impl ContextContractFilter {
    fn new() -> Self {
        Self {
            before: Mutex::new(None),
            after_count: AtomicUsize::new(0),
            row_counts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl BeforeFilter for ContextContractFilter {
    fn name(&self) -> &str {
        "context-contract"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        *self.before.lock().expect("before snapshot lock poisoned") = Some(BeforeSnapshot {
            sql: context.sql.to_string(),
            params: context.params.to_vec(),
            data_source: context.data_source.to_string(),
            start: context.start,
        });
        context.fingerprint = Some(0xD2_01_D2_01);
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for ContextContractFilter {
    fn name(&self) -> &str {
        "context-contract"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) {
        // 主调用必须 await after；主动让出一次调度可捕获“创建 future 但未轮询”的回归。
        tokio::task::yield_now().await;
        let before = self
            .before
            .lock()
            .expect("before snapshot lock poisoned")
            .clone()
            .expect("after must observe a preceding before");
        assert_eq!(context.sql, before.sql);
        assert_eq!(context.params, before.params);
        assert_eq!(context.data_source, before.data_source);
        assert_eq!(context.start, before.start);
        assert_eq!(context.fingerprint, Some(0xD2_01_D2_01));
        assert_eq!(context.data_source, "contract-data-source");

        let row_count = result.as_ref().ok().and_then(|value| value.row_count);
        self.row_counts
            .lock()
            .expect("row count lock poisoned")
            .push(row_count);
        self.after_count.fetch_add(1, Ordering::Release);
    }
}

async fn contract_pool(filter: Arc<ContextContractFilter>) -> DruidPool {
    let mut filter_chain = FilterChain::new();
    filter_chain.add_before(filter.clone());
    filter_chain.add_after(filter);
    DruidPool::builder()
        .name("contract-data-source")
        .driver_name("contract")
        .factory(Arc::new(ContractFactory))
        .max_open(2)
        .max_idle(2)
        .filter_chain(Arc::new(filter_chain))
        .build()
        .await
        .expect("contract pool must build")
}

#[tokio::test]
async fn exec_and_fetch_preserve_one_filter_context() {
    let filter = Arc::new(ContextContractFilter::new());
    let pool = contract_pool(filter.clone()).await;
    let mut connection = pool
        .get()
        .await
        .expect("connection acquisition must succeed");

    let result = connection
        .exec("UPDATE account SET balance = ?", vec![Value::Int(7)])
        .await
        .expect("exec must succeed");
    assert_eq!(result.rows_affected, 1);
    assert_eq!(filter.after_count.load(Ordering::Acquire), 1);

    let rows = connection
        .fetch(
            "SELECT id FROM account WHERE owner = ?",
            vec![Value::String("alice".to_string())],
        )
        .await
        .expect("fetch must succeed");
    assert_eq!(rows.len(), 2);
    assert_eq!(filter.after_count.load(Ordering::Acquire), 2);
    assert_eq!(
        *filter.row_counts.lock().expect("row count lock poisoned"),
        vec![None, Some(2)]
    );
}

#[tokio::test]
async fn explicit_close_and_drop_recycle_exactly_once() {
    let filter = Arc::new(ContextContractFilter::new());
    let pool = contract_pool(filter).await;
    let mut connection = pool
        .get()
        .await
        .expect("connection acquisition must succeed");

    connection.close().await.expect("first close must recycle");
    connection
        .close()
        .await
        .expect("duplicate close must be idempotent");
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    assert_eq!(pool.state().recycle_count, 1);

    drop(connection);
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 1);
    assert_eq!(pool.state().recycle_count, 1);
}

#[test]
fn unwind_drop_recycles_exactly_once() {
    let recycle_count = Arc::new(AtomicUsize::new(0));
    let observed_recycle_count = recycle_count.clone();

    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _connection = druid::core::DruidPooledConnection::new(
            Box::new(ContractPhysicalConnection),
            41,
            Box::new(move |_connection, connection_id| {
                assert_eq!(connection_id, 41);
                observed_recycle_count.fetch_add(1, Ordering::Relaxed);
            }),
        );
        panic!("contract panic");
    }));

    assert!(unwind.is_err());
    assert_eq!(recycle_count.load(Ordering::Relaxed), 1);
}

struct GatedFactory {
    entered: AtomicUsize,
    entered_notify: Notify,
    release: Semaphore,
}

#[async_trait::async_trait]
impl ConnectionFactory for GatedFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        self.entered.fetch_add(1, Ordering::AcqRel);
        self.entered_notify.notify_one();
        self.release
            .acquire()
            .await
            .expect("release semaphore must remain open")
            .forget();
        Ok(Box::new(ContractPhysicalConnection))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_creation_never_exceeds_max_open() {
    let factory = Arc::new(GatedFactory {
        entered: AtomicUsize::new(0),
        entered_notify: Notify::new(),
        release: Semaphore::new(0),
    });
    let pool = Arc::new(
        DruidPool::builder()
            .name("capacity-contract")
            .driver_name("contract")
            .factory(factory.clone())
            .max_open(2)
            .max_idle(2)
            .acquire_timeout(Duration::from_secs(2))
            .build()
            .await
            .expect("capacity pool must build"),
    );

    let mut tasks = Vec::new();
    for _ in 0..32 {
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            let connection = pool.get().await.expect("acquisition must succeed");
            drop(connection);
        }));
    }

    tokio::time::timeout(Duration::from_secs(1), async {
        while factory.entered.load(Ordering::Acquire) < 2 {
            factory.entered_notify.notified().await;
        }
    })
    .await
    .expect("two physical creations must enter");
    assert_eq!(factory.entered.load(Ordering::Acquire), 2);
    factory.release.add_permits(2);

    for task in tasks {
        task.await.expect("acquisition task must not panic");
    }

    assert_eq!(factory.entered.load(Ordering::Acquire), 2);
    assert_eq!(pool.state().create_count, 2);
    assert_eq!(pool.state().active_count, 0);
}
