//! Java Druid 空闲维护与物理寿命语义测试。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 对照实现：`DruidDataSource#recycle`、`DruidDataSource#shrink`。
//! 对照测试：`DruidDataSourceShrinkTest`、`MaxPhyTimeMillisTest`。

use druid::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionFactory, Row,
    ValidConnectionChecker, Value,
};
use druid::pool::DruidPool;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct MaintenanceConnection {
    closed_count: Arc<AtomicU64>,
    discarded: bool,
    closed: bool,
}

#[async_trait::async_trait]
impl PhysicalConnection for MaintenanceConnection {
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

    fn driver_name(&self) -> &str {
        "maintenance"
    }
}

struct MaintenanceFactory {
    create_count: AtomicU64,
    validate_count: Arc<AtomicU64>,
    validation_succeeds: Arc<AtomicBool>,
    closed_count: Arc<AtomicU64>,
}

impl MaintenanceFactory {
    fn new() -> Self {
        Self {
            create_count: AtomicU64::new(0),
            validate_count: Arc::new(AtomicU64::new(0)),
            validation_succeeds: Arc::new(AtomicBool::new(true)),
            closed_count: Arc::new(AtomicU64::new(0)),
        }
    }

    fn checker(&self) -> Arc<dyn ValidConnectionChecker> {
        Arc::new(MaintenanceChecker {
            validate_count: Arc::clone(&self.validate_count),
            validation_succeeds: Arc::clone(&self.validation_succeeds),
        })
    }
}

struct MaintenanceChecker {
    validate_count: Arc<AtomicU64>,
    validation_succeeds: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl ValidConnectionChecker for MaintenanceChecker {
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
impl PhysicalConnectionFactory for MaintenanceFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        self.create_count.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(MaintenanceConnection {
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
                "maintenance validation failed".to_string(),
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

#[tokio::test]
async fn shrink_without_time_keeps_exact_java_min_idle_boundary() {
    let factory = Arc::new(MaintenanceFactory::new());
    let pool = DruidPool::builder()
        .name("shrink-no-time")
        .driver_name("maintenance")
        .factory(factory)
        .max_open(8)
        .max_idle(8)
        .min_idle(3)
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 8).await;
    assert_eq!(pool.state().idle_count, 8);

    pool.shrink().await;

    assert_eq!(pool.state().idle_count, 3);
    assert_eq!(pool.state().destroy_count, 5);
}

#[tokio::test]
async fn max_phy_time_test_matches_java_shrink_below_min_idle() {
    let factory = Arc::new(MaintenanceFactory::new());
    let pool = DruidPool::builder()
        .name("max-phy-time")
        .driver_name("maintenance")
        .factory(factory)
        .max_open(10)
        .max_idle(10)
        .min_idle(5)
        .idle_timeout(Duration::from_millis(10))
        .physical_connection_timeout(Duration::from_millis(100))
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 10).await;
    assert_eq!(pool.state().idle_count, 10);

    tokio::time::sleep(Duration::from_millis(20)).await;
    pool.shrink_with_options(true, false).await;
    assert_eq!(pool.state().idle_count, 5);

    tokio::time::sleep(Duration::from_millis(90)).await;
    pool.shrink_with_options(true, false).await;
    assert_eq!(pool.state().idle_count, 0);
    assert_eq!(pool.state().destroy_count, 10);
}

#[tokio::test]
async fn recycle_discards_connection_older_than_java_phy_timeout() {
    let factory = Arc::new(MaintenanceFactory::new());
    let pool = DruidPool::builder()
        .name("recycle-phy-timeout")
        .driver_name("maintenance")
        .factory(factory)
        .max_open(1)
        .max_idle(1)
        .physical_connection_timeout(Duration::from_millis(5))
        .build()
        .await
        .unwrap();

    let mut connection = pool.get().await.unwrap();
    tokio::time::sleep(Duration::from_millis(10)).await;
    connection.close().await.unwrap();

    assert_eq!(pool.state().idle_count, 0);
    assert_eq!(pool.state().discard_count, 1);
    assert_eq!(pool.state().destroy_count, 1);
}

#[tokio::test]
async fn keep_alive_validates_due_connections_and_preserves_queue_order() {
    let factory = Arc::new(MaintenanceFactory::new());
    let pool = DruidPool::builder()
        .name("keep-alive")
        .driver_name("maintenance")
        .factory(factory.clone())
        .max_open(2)
        .max_idle(2)
        .min_idle(2)
        .idle_timeout(Duration::from_secs(60))
        .time_between_eviction_runs(Duration::ZERO)
        .keep_alive(true)
        .keep_alive_between_time(Duration::from_millis(5))
        .valid_connection_checker(factory.checker())
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 2).await;
    let validate_before = factory.validate_count.load(Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(10)).await;
    pool.shrink_check_time(true).await;

    assert_eq!(pool.state().idle_count, 2);
    assert_eq!(pool.state().keep_alive_check_count, 2);
    assert_eq!(pool.state().keep_alive_check_error_count, 0);
    assert_eq!(
        factory.validate_count.load(Ordering::Relaxed) - validate_before,
        2
    );

    let first_id = pool.get().await.unwrap().id();
    let second_id = pool.get().await.unwrap().id();
    assert!(first_id < second_id, "keepAlive 必须保留原空闲队列顺序");
}

#[tokio::test]
async fn keep_alive_failure_discards_and_counts_each_physical_connection() {
    let factory = Arc::new(MaintenanceFactory::new());
    let pool = DruidPool::builder()
        .name("keep-alive-failure")
        .driver_name("maintenance")
        .factory(factory.clone())
        .max_open(2)
        .max_idle(2)
        .min_idle(0)
        .idle_timeout(Duration::from_secs(60))
        .keep_alive_between_time(Duration::from_millis(5))
        .valid_connection_checker(factory.checker())
        .build()
        .await
        .unwrap();

    fill_idle(&pool, 2).await;
    factory.validation_succeeds.store(false, Ordering::Relaxed);
    tokio::time::sleep(Duration::from_millis(10)).await;
    pool.shrink_with_options(true, true).await;

    assert_eq!(pool.state().idle_count, 0);
    assert_eq!(pool.state().keep_alive_check_count, 2);
    assert_eq!(pool.state().keep_alive_check_error_count, 2);
    assert_eq!(pool.state().discard_count, 2);
    assert_eq!(pool.state().destroy_count, 2);
    assert_eq!(factory.closed_count.load(Ordering::Relaxed), 2);
}
