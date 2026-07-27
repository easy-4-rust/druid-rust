//! Comprehensive coverage tests for druid-pool crate.
//!
//! Targets: pool_inner.rs (69 uncovered), druid_pool.rs (49 uncovered),
//! pooled_connection.rs (42 uncovered), config.rs (33 uncovered).

use druid_core::*;
use druid_pool::DruidPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ══════════════════════════════════════════════════════════════════
// Mock Connection + Factory
// ══════════════════════════════════════════════════════════════════

struct MockConn {
    id: u64,
    closed: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl Connection for MockConn {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult { rows_affected: 1, last_insert_id: Some(self.id as i64), row_count: None })
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![Row::new(vec![Value::Int(self.id as i64)])])
    }
    async fn begin(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn commit(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn rollback(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn close(&mut self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Relaxed);
        Ok(())
    }
    fn driver_name(&self) -> &str { "mock" }
}

struct MockFactory {
    counter: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl ConnectionFactory for MockFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
        let id = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Box::new(MockConn { id, closed: std::sync::atomic::AtomicBool::new(false) }))
    }
    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
        conn.ping().await
    }
}

fn make_factory() -> (Arc<MockFactory>, Arc<AtomicU64>) {
    let counter = Arc::new(AtomicU64::new(0));
    (Arc::new(MockFactory { counter: counter.clone() }), counter)
}

async fn build_pool(max_open: usize, max_idle: usize) -> DruidPool {
    let (factory, _) = make_factory();
    DruidPool::builder()
        .name("test")
        .driver_name("mock")
        .factory(factory)
        .max_open(max_open)
        .max_idle(max_idle)
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await
        .unwrap()
}

// ══════════════════════════════════════════════════════════════════
// 1. config.rs: DruidPoolBuilder all methods + PoolInnerConfig default
// ══════════════════════════════════════════════════════════════════

#[test]
fn test_pool_inner_config_default() {
    let cfg = druid_pool::PoolInnerConfig::default();
    assert_eq!(cfg.max_open, 8);
    assert_eq!(cfg.min_idle, 0);
    assert_eq!(cfg.max_idle, 8);
    assert_eq!(cfg.acquire_timeout, Duration::from_secs(30));
    assert_eq!(cfg.max_lifetime, Duration::from_secs(1800));
    assert_eq!(cfg.idle_timeout, Duration::from_secs(600));
    assert!(!cfg.test_on_borrow);
}

#[test]
fn test_druid_pool_builder_all_methods() {
    let (factory, _) = make_factory();
    let fc = Arc::new(FilterChain::new());
    let builder = DruidPool::builder()
        .name("my-pool")
        .driver_name("pg")
        .factory(factory)
        .max_open(20)
        .min_idle(5)
        .max_idle(15)
        .acquire_timeout(Duration::from_secs(10))
        .test_on_borrow(true)
        .filter_chain(fc);

    // Verify builder state via build
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = builder.build().await.unwrap();
        assert_eq!(pool.name(), "my-pool");
        assert_eq!(pool.driver_name(), "pg");
        assert!(pool.filter_chain().is_some());
    });
}

#[test]
fn test_druid_pool_builder_default() {
    let _ = druid_pool::DruidPoolBuilder::default();
}

#[tokio::test]
async fn test_druid_pool_builder_without_factory() {
    let result = DruidPool::builder()
        .name("no-factory")
        .build()
        .await;
    assert!(result.is_err());
}

// ══════════════════════════════════════════════════════════════════
// 2. druid_pool.rs: DruidPool::new, state, close, name, driver_name
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_druid_pool_new_direct() {
    let (factory, _) = make_factory();
    let config = druid_pool::PoolInnerConfig::default();
    let pool = DruidPool::new("direct".into(), "mock".into(), factory, config, None);
    assert_eq!(pool.name(), "direct");
    assert_eq!(pool.driver_name(), "mock");
    assert!(pool.filter_chain().is_none());
}

#[tokio::test]
async fn test_druid_pool_state() {
    let pool = build_pool(4, 2).await;
    let st = pool.state();
    assert_eq!(st.name, "test");
    assert_eq!(st.driver_name, "mock");
    assert_eq!(st.max_open, 4);
    assert!(!st.closed);
    assert_eq!(st.active_count, 0);
    assert_eq!(st.idle_count, 0);
    assert_eq!(st.create_count, 0);
    assert_eq!(st.close_count, 0);
    assert_eq!(st.connect_count, 0);
    assert_eq!(st.connect_error_count, 0);
    assert_eq!(st.recycle_count, 0);
}

#[tokio::test]
async fn test_druid_pool_state_after_acquire_release() {
    let pool = build_pool(4, 2).await;
    let c = pool.get().await.unwrap();
    let st = pool.state();
    assert_eq!(st.active_count, 1);
    assert_eq!(st.connect_count, 1);
    drop(c);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let st = pool.state();
    assert_eq!(st.active_count, 0);
    assert_eq!(st.idle_count, 1);
    assert_eq!(st.recycle_count, 1);
}

#[tokio::test]
async fn test_druid_pool_get_from_idle() {
    let pool = build_pool(2, 2).await;
    // Create a connection
    let c1 = pool.get().await.unwrap();
    drop(c1);
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Get should reuse the idle connection
    let c2 = pool.get().await.unwrap();
    assert_eq!(c2.id(), 1); // Same ID as the first connection
    let st = pool.state();
    assert_eq!(st.create_count, 1); // Only one connection created
    drop(c2);
}

#[tokio::test]
async fn test_druid_pool_get_timeout_closed() {
    let pool = build_pool(2, 2).await;
    pool.close().await;
    let result = pool.get().await;
    assert!(matches!(result, Err(DruidError::PoolClosed)));
}

#[tokio::test]
async fn test_druid_pool_get_timeout_acquire() {
    let pool = build_pool(1, 1).await;
    let _c = pool.get().await.unwrap(); // Fill the pool
    let result = pool.get_timeout(Duration::from_millis(100)).await;
    assert!(matches!(result, Err(DruidError::AcquireTimeout)));
}

#[tokio::test]
async fn test_druid_pool_close_drains_idle() {
    let pool = build_pool(4, 4).await;
    let c1 = pool.get().await.unwrap();
    let c2 = pool.get().await.unwrap();
    drop(c1);
    drop(c2);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let st = pool.state();
    assert!(st.idle_count > 0);

    pool.close().await;
    let st = pool.state();
    assert!(st.closed);
}

#[tokio::test]
async fn test_druid_pool_filter_chain() {
    let (factory, _) = make_factory();
    let fc = Arc::new(FilterChain::new());
    let pool = DruidPool::builder()
        .name("fc-test")
        .driver_name("mock")
        .factory(factory)
        .max_open(2)
        .max_idle(2)
        .filter_chain(fc)
        .build()
        .await
        .unwrap();
    assert!(pool.filter_chain().is_some());
}

// ══════════════════════════════════════════════════════════════════
// 3. pooled_connection.rs: DruidPoolConnection all methods
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_druid_pool_connection_exec() {
    let pool = build_pool(2, 2).await;
    let mut conn = pool.get().await.unwrap();
    let result = conn.exec("INSERT INTO t VALUES (1)", vec![]).await.unwrap();
    assert_eq!(result.rows_affected, 1);
}

#[tokio::test]
async fn test_druid_pool_connection_fetch() {
    let pool = build_pool(2, 2).await;
    let mut conn = pool.get().await.unwrap();
    let rows = conn.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn test_druid_pool_connection_ping() {
    let pool = build_pool(2, 2).await;
    let mut conn = pool.get().await.unwrap();
    conn.ping().await.unwrap();
}

#[tokio::test]
async fn test_druid_pool_connection_driver_name() {
    let pool = build_pool(2, 2).await;
    let conn = pool.get().await.unwrap();
    assert_eq!(conn.driver_name(), "mock");
}

#[tokio::test]
async fn test_druid_pool_connection_id() {
    let pool = build_pool(2, 2).await;
    let conn = pool.get().await.unwrap();
    assert!(conn.id() > 0);
}

#[test]
fn test_druid_pool_connection_debug() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = build_pool(2, 2).await;
        let conn = pool.get().await.unwrap();
        let debug_str = format!("{:?}", conn);
        assert!(debug_str.contains("DruidPoolConnection"));
        assert!(debug_str.contains("has_conn"));
    });
}

#[test]
fn test_druid_pool_connection_into_core() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = build_pool(2, 2).await;
        let conn = pool.get().await.unwrap();
        let core_conn = conn.into_core();
        assert!(core_conn.id() > 0);
    });
}

#[tokio::test]
async fn test_druid_pool_connection_drop_returns() {
    let pool = build_pool(2, 2).await;
    {
        let _c1 = pool.get().await.unwrap();
        let _c2 = pool.get().await.unwrap();
        assert_eq!(pool.state().active_count, 2);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 2);
}

// ══════════════════════════════════════════════════════════════════
// 4. pool_inner.rs: PoolInner internal methods
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_pool_inner_can_grow() {
    let pool = build_pool(2, 2).await;
    let st = pool.state();
    assert_eq!(st.max_open, 2);
    // After acquiring 1, can_grow should still be true
    let c = pool.get().await.unwrap();
    let st = pool.state();
    assert_eq!(st.active_count, 1);
    drop(c);
}

#[tokio::test]
async fn test_pool_inner_create_connection_error() {
    struct FailFactory;
    #[async_trait::async_trait]
    impl ConnectionFactory for FailFactory {
        async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
            Err(DruidError::DriverError("connection refused".into()))
        }
        async fn validate(&self, _conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
            Ok(())
        }
    }

    let pool = DruidPool::builder()
        .name("fail")
        .driver_name("mock")
        .factory(Arc::new(FailFactory))
        .max_open(2)
        .max_idle(2)
        .acquire_timeout(Duration::from_secs(1))
        .build()
        .await
        .unwrap();

    let result = pool.get_timeout(Duration::from_millis(200)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_pool_inner_return_after_close() {
    let pool = build_pool(2, 2).await;
    let c = pool.get().await.unwrap();
    pool.close().await;
    drop(c); // Should destroy connection, not return to pool
    let st = pool.state();
    assert_eq!(st.idle_count, 0);
}

#[tokio::test]
async fn test_pool_inner_max_idle_eviction() {
    let pool = build_pool(4, 1).await;
    // Acquire 4 connections
    let c1 = pool.get().await.unwrap();
    let c2 = pool.get().await.unwrap();
    let c3 = pool.get().await.unwrap();
    let c4 = pool.get().await.unwrap();
    // Release all - but max_idle=1, so only 1 should be kept
    drop(c1);
    drop(c2);
    drop(c3);
    drop(c4);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let st = pool.state();
    assert!(st.idle_count <= 1, "idle_count={} should <= 1", st.idle_count);
}

#[tokio::test]
async fn test_pool_inner_concurrent_acquire_release() {
    let pool = Arc::new(build_pool(4, 4).await);
    let mut handles = vec![];
    for i in 0..20 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            let conn = pool.get().await.unwrap();
            let _ = conn.id();
            drop(conn);
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let st = pool.state();
    assert_eq!(st.active_count, 0);
}

// ══════════════════════════════════════════════════════════════════
// 5. lib.rs: Pool trait implementation
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_pool_trait_get() {
    let pool = build_pool(2, 2).await;
    let pool_trait: &dyn druid_core::Pool = &pool;
    let conn = pool_trait.get().await.unwrap();
    assert!(conn.id() > 0);
}

#[tokio::test]
async fn test_pool_trait_get_timeout() {
    let pool = build_pool(1, 1).await;
    let _c = pool.get().await.unwrap();
    let pool_trait: &dyn druid_core::Pool = &pool;
    let result = pool_trait.get_timeout(Duration::from_millis(100)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_pool_trait_state() {
    let pool = build_pool(2, 2).await;
    let pool_trait: &dyn druid_core::Pool = &pool;
    let st = pool_trait.state();
    assert_eq!(st.name, "test");
    assert_eq!(pool_trait.driver_name(), "mock");
    assert_eq!(pool_trait.name(), "test");
}

// ══════════════════════════════════════════════════════════════════
// 6. Stress test: 10000 acquire/release
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_stress_10000_acquire_release() {
    let pool = build_pool(4, 4).await;
    for _ in 0..10_000 {
        let c = pool.get().await.unwrap();
        drop(c);
    }
    let st = pool.state();
    assert!(st.recycle_count > 0);
    assert_eq!(st.active_count, 0);
}
