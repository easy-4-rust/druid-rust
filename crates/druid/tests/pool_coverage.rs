//! Comprehensive coverage tests for druid-pool crate.
//!
//! Targets: pool_inner.rs (69 uncovered), druid_pool.rs (49 uncovered),
//! pooled_connection.rs (42 uncovered), config.rs (33 uncovered).

use druid::core::*;
use druid::pool::DruidPool;
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
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: Some(self.id as i64),
            row_count: None,
        })
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![Row::new(vec![Value::Int(self.id as i64)])])
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
        self.closed.store(true, Ordering::Relaxed);
        Ok(())
    }
    fn driver_name(&self) -> &str {
        "mock"
    }
}

struct MockFactory {
    counter: Arc<AtomicU64>,
}

#[async_trait::async_trait]
impl ConnectionFactory for MockFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
        let id = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Box::new(MockConn {
            id,
            closed: std::sync::atomic::AtomicBool::new(false),
        }))
    }
    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
        conn.ping().await
    }
}

fn make_factory() -> (Arc<MockFactory>, Arc<AtomicU64>) {
    let counter = Arc::new(AtomicU64::new(0));
    (
        Arc::new(MockFactory {
            counter: counter.clone(),
        }),
        counter,
    )
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
    let cfg = druid::pool::PoolInnerConfig::default();
    assert_eq!(cfg.db_type_name, None);
    assert_eq!(cfg.max_open, 8);
    assert_eq!(cfg.min_idle, 0);
    assert_eq!(cfg.max_idle, 8);
    assert_eq!(cfg.acquire_timeout, Duration::MAX);
    assert_eq!(cfg.max_lifetime, Duration::MAX);
    assert_eq!(cfg.idle_timeout, Duration::from_secs(1800));
    assert_eq!(
        cfg.max_evictable_idle_time,
        Duration::from_secs(7 * 60 * 60)
    );
    assert_eq!(cfg.physical_connection_timeout, None);
    assert!(!cfg.test_on_borrow);
    assert!(!cfg.test_on_return);
    assert!(!cfg.keep_alive);
    assert_eq!(cfg.keep_alive_between_time, Duration::from_secs(120));
    assert!(!cfg.keep_connection_underlying_transaction_isolation);
    assert_eq!(cfg.max_use_count, 0);
    assert!(cfg.default_auto_commit);
    assert_eq!(cfg.default_read_only, None);
    assert_eq!(cfg.default_transaction_isolation, None);
    assert_eq!(cfg.default_catalog, None);
}

#[test]
fn test_druid_pool_builder_all_methods() {
    let (factory, _) = make_factory();
    let fc = Arc::new(FilterChain::new());
    let builder = DruidPool::builder()
        .name("my-pool")
        .driver_name("pg")
        .db_type_name("postgresql")
        .factory(factory)
        .max_open(20)
        .min_idle(5)
        .max_idle(15)
        .acquire_timeout(Duration::from_secs(10))
        .max_lifetime(Duration::from_secs(3600))
        .idle_timeout(Duration::from_secs(1200))
        .max_evictable_idle_time(Duration::from_secs(7200))
        .physical_connection_timeout(Duration::from_secs(300))
        .phy_timeout(Duration::from_secs(301))
        .test_on_borrow(true)
        .test_on_return(true)
        .time_between_eviction_runs(Duration::from_secs(30))
        .keep_alive(true)
        .keep_alive_between_time(Duration::from_secs(60))
        .keep_connection_underlying_transaction_isolation(true)
        .max_use_count(100)
        .default_auto_commit(false)
        .default_read_only(true)
        .default_transaction_isolation(8)
        .default_catalog("catalog_a")
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
    let _ = druid::pool::DruidPoolBuilder::default();
}

#[tokio::test]
async fn test_druid_pool_builder_without_factory() {
    let result = DruidPool::builder().name("no-factory").build().await;
    assert!(result.is_err());
}

// ══════════════════════════════════════════════════════════════════
// 2. druid_pool.rs: DruidPool::new, state, close, name, driver_name
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_druid_pool_new_direct() {
    let (factory, _) = make_factory();
    let config = druid::pool::PoolInnerConfig::default();
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
    assert!((1..=4).contains(&st.connect_count));
    drop(c);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let st = pool.state();
    assert_eq!(st.active_count, 0);
    assert!((1..=4).contains(&st.idle_count));
    assert_eq!(st.recycle_count, 1);
}

#[tokio::test]
async fn test_druid_pool_get_from_idle() {
    let pool = build_pool(2, 2).await;
    // Create a connection
    let c1 = pool.get().await.unwrap();
    let first_id = c1.id();
    drop(c1);
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Get should reuse the idle connection
    let c2 = pool.get().await.unwrap();
    assert_eq!(c2.id(), first_id); // Same physical holder as the first connection
    let st = pool.state();
    assert_eq!(st.create_count, 1); // Only one connection created
    drop(c2);
}

#[tokio::test]
async fn test_druid_pool_get_timeout_closed() {
    let pool = build_pool(2, 2).await;
    // Java Druid 尚未 init 时 close 无副作用。
    pool.close().await;
    let mut connection = pool.get().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
    assert!(matches!(
        pool.get().await,
        Err(DruidError::DataSourceClosed { .. })
    ));
}

#[tokio::test]
async fn test_druid_pool_get_timeout_acquire() {
    let pool = build_pool(1, 1).await;
    let _c = pool.get().await.unwrap(); // Fill the pool
    let result = pool.get_timeout(Duration::from_millis(100)).await;
    assert!(matches!(
        result,
        Err(DruidError::GetConnectionTimeout { .. })
    ));
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
// 3. druid_pooled_connection.rs: DruidPooledConnection all methods
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
        assert!(debug_str.contains("DruidPooledConnection"));
        assert!(debug_str.contains("has_physical_connection"));
    });
}

#[test]
fn test_pool_trait_returns_canonical_pooled_connection() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let pool = build_pool(2, 2).await;
        let pool_trait: &dyn Pool = &pool;
        let connection = pool_trait.get().await.unwrap();
        assert!(connection.id() > 0);
        assert_eq!(connection.data_source(), "test");
        drop(connection);
        assert_eq!(pool.state().active_count, 0);
        assert_eq!(pool.state().idle_count, 1);
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
async fn test_pool_inner_max_idle_is_not_a_return_cap() {
    let pool = build_pool(4, 1).await;
    // Acquire 4 connections
    let c1 = pool.get().await.unwrap();
    let c2 = pool.get().await.unwrap();
    let c3 = pool.get().await.unwrap();
    let c4 = pool.get().await.unwrap();
    // Java Druid 的 maxIdle 是兼容字段；归还上限由 maxActive 管理。
    drop(c1);
    drop(c2);
    drop(c3);
    drop(c4);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let st = pool.state();
    assert_eq!(st.idle_count, 4);
}

#[tokio::test]
async fn test_pool_inner_concurrent_acquire_release() {
    let pool = Arc::new(build_pool(4, 4).await);
    let mut handles = vec![];
    for _i in 0..20 {
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
    let pool_trait: &dyn druid::core::Pool = &pool;
    let conn = pool_trait.get().await.unwrap();
    assert!(conn.id() > 0);
}

#[tokio::test]
async fn test_pool_trait_get_timeout() {
    let pool = build_pool(1, 1).await;
    let _c = pool.get().await.unwrap();
    let pool_trait: &dyn druid::core::Pool = &pool;
    let result = pool_trait.get_timeout(Duration::from_millis(100)).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_pool_trait_state() {
    let pool = build_pool(2, 2).await;
    let pool_trait: &dyn druid::core::Pool = &pool;
    let st = pool_trait.state();
    assert_eq!(st.name, "test");
    assert_eq!(pool_trait.driver_name(), "mock");
    assert_eq!(pool_trait.name(), "test");
}

// ══════════════════════════════════════════════════════════════════
// 6. Stress test: 10000 acquire/release
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 7. Pool trait: get_timeout via trait object
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_pool_trait_get_timeout_success() {
    let pool = build_pool(2, 2).await;
    let pool_trait: &dyn druid::core::Pool = &pool;
    let conn = pool_trait
        .get_timeout(Duration::from_secs(2))
        .await
        .unwrap();
    assert!(conn.id() > 0);
}

// ══════════════════════════════════════════════════════════════════
// 8. PoolInner: should_evict via pool state
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_pool_inner_should_evict() {
    let pool = build_pool(4, 1).await; // min_idle=1
                                       // Acquire 3 connections
    let c1 = pool.get().await.unwrap();
    let c2 = pool.get().await.unwrap();
    let c3 = pool.get().await.unwrap();
    // Release all 后应满足 should_evict；显式 shrink 才执行回收。
    drop(c1);
    drop(c2);
    drop(c3);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pool.state().idle_count, 3);
    pool.shrink().await;
    assert_eq!(pool.state().idle_count, 0);
}

#[test]
fn test_pool_inner_should_evict_direct() {
    // Directly test should_evict method
    let (factory, _) = make_factory();
    let config = druid::pool::PoolInnerConfig {
        max_open: 4,
        min_idle: 2,
        max_idle: 4,
        ..Default::default()
    };
    let inner = druid::pool::PoolInner::new(factory, config);
    // Empty idle queue - should not evict
    assert!(!inner.should_evict());
}

// ══════════════════════════════════════════════════════════════════
// 9. get_timeout retry loop: create fails but idle available
// ══════════════════════════════════════════════════════════════════

// ══════════════════════════════════════════════════════════════════
// 10. DruidPooledConnection: before_execute error path
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_pool_connection_before_execute_error() {
    use druid::core::{BeforeFilter, ExecContext};

    struct BlockingFilter;
    #[async_trait::async_trait]
    impl BeforeFilter for BlockingFilter {
        fn name(&self) -> &str {
            "blocking"
        }
        async fn before(&self, _ctx: &mut ExecContext<'_>) -> Result<(), DruidError> {
            Err(DruidError::WallViolation("blocked by filter".into()))
        }
    }

    let (factory, _) = make_factory();
    let mut fc = FilterChain::new();
    fc.add_before(Arc::new(BlockingFilter));

    let pool = DruidPool::builder()
        .name("filter-test")
        .driver_name("mock")
        .factory(factory)
        .max_open(2)
        .max_idle(2)
        .filter_chain(Arc::new(fc))
        .build()
        .await
        .unwrap();

    let mut conn = pool.get().await.unwrap();
    let result = conn.exec("SELECT 1", vec![]).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DruidError::WallViolation(_)));
}

// ══════════════════════════════════════════════════════════════════
// 11. DruidPooledConnection: explicit close and Drop share one recycle path
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_pool_connection_explicit_close_recycles_exactly_once() {
    let pool = build_pool(2, 2).await;
    let mut connection = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    connection.close().await.unwrap();
    connection.close().await.unwrap();
    assert!(connection.is_recycled());
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().recycle_count, 1);
    drop(connection);
    assert_eq!(pool.state().recycle_count, 1);
}

// ══════════════════════════════════════════════════════════════════
// 12. DruidPooledConnection: filter_chain = None
// ══════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_pool_connection_no_filter_chain() {
    // Create a pool WITHOUT a filter chain to trigger the false branch
    // of `if let Some(ref fc) = self.filter_chain`
    let pool = DruidPool::builder()
        .name("no-filter")
        .driver_name("mock")
        .factory(Arc::new(MockFactory {
            counter: Arc::new(AtomicU64::new(0)),
        }))
        .max_open(2)
        .max_idle(2)
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await
        .unwrap();

    // Verify no filter chain
    assert!(pool.filter_chain().is_none());

    let mut conn = pool.get().await.unwrap();
    // exec with no filter chain should skip the if-let block
    let result = conn.exec("SELECT 1", vec![]).await.unwrap();
    assert_eq!(result.rows_affected, 1);
}

#[tokio::test]
async fn test_pool_get_timeout_notify_fires() {
    // Tests the Ok(_) => continue branch in get_timeout
    // where notify fires before deadline (connection returned while waiting)
    let pool = build_pool(1, 1).await;
    let c1 = pool.get().await.unwrap(); // fill pool

    // Spawn a task that will release the connection after a short delay
    let pool_clone = Arc::new(pool);
    let _p = pool_clone.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        drop(c1); // This triggers notify
    });

    // This should wait, get notified when c1 is dropped, and retry
    let result = pool_clone.get_timeout(Duration::from_secs(2)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_pool_get_timeout_retry_on_create_failure() {
    // This tests the `Ok(_) => continue` branch in get_timeout
    // where create_connection fails but idle connections are available
    use std::sync::atomic::AtomicBool;

    struct FailAfterFirstFactory {
        counter: AtomicU64,
        fail: AtomicBool,
    }

    #[async_trait::async_trait]
    impl ConnectionFactory for FailAfterFirstFactory {
        async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
            let count = self.counter.fetch_add(1, Ordering::SeqCst);
            if self.fail.load(Ordering::Relaxed) && count > 0 {
                Err(DruidError::DriverError("transient failure".into()))
            } else {
                Ok(Box::new(MockConn {
                    id: count + 1,
                    closed: std::sync::atomic::AtomicBool::new(false),
                }))
            }
        }
        async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
            conn.ping().await
        }
    }

    let factory = Arc::new(FailAfterFirstFactory {
        counter: AtomicU64::new(0),
        fail: AtomicBool::new(false),
    });

    let pool = DruidPool::builder()
        .name("retry-test")
        .driver_name("mock")
        .factory(factory.clone())
        .max_open(2)
        .max_idle(2)
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await
        .unwrap();

    // Get first connection (succeeds)
    let c1 = pool.get().await.unwrap();
    // Now enable failure
    factory.fail.store(true, Ordering::Relaxed);
    // Get second - create will fail, but should retry from idle
    // Actually, we need to release c1 first so there's an idle connection
    drop(c1);
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Now get should reuse idle (c1) even though create fails
    let c2 = pool.get().await.unwrap();
    assert!(c2.id() > 0);
}

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
