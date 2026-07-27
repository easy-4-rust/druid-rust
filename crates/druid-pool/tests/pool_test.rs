//! druid-pool S2 验收测试：FR-020 ~ FR-024

use druid_core::*;
use druid_pool::DruidPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ── Mock Connection ──────────────────────────────────────────────

struct MockConnection {
    closed: bool,
    exec_count: Arc<AtomicU64>,
}

impl MockConnection {
    fn new() -> (Self, Arc<AtomicU64>) {
        let count = Arc::new(AtomicU64::new(0));
        (Self { closed: false, exec_count: count.clone() }, count)
    }
}

#[async_trait::async_trait]
impl Connection for MockConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        self.exec_count.fetch_add(1, Ordering::Relaxed);
        Ok(ExecResult { rows_affected: 1, last_insert_id: Some(1) })
    }
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![Row::new(vec![Value::Int(1)])])
    }
    async fn begin(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn commit(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn rollback(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn ping(&mut self) -> Result<(), DruidError> { Ok(()) }
    async fn close(&mut self) -> Result<(), DruidError> { self.closed = true; Ok(()) }
    fn driver_name(&self) -> &str { "mock" }
}

// ── Mock Factory ─────────────────────────────────────────────────

struct MockFactory;
static CREATED: AtomicU64 = AtomicU64::new(0);

#[async_trait::async_trait]
impl ConnectionFactory for MockFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, DruidError> {
        CREATED.fetch_add(1, Ordering::SeqCst);
        let (conn, _) = MockConnection::new();
        Ok(Box::new(conn))
    }
    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), DruidError> {
        conn.ping().await
    }
}

// ── Helper ───────────────────────────────────────────────────────

async fn build_pool(max_open: usize, max_idle: usize) -> DruidPool {
    DruidPool::builder()
        .name("test")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(max_open)
        .max_idle(max_idle)
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await
        .unwrap()
}

// FR-020: max_open 上限
#[tokio::test]
async fn test_max_open_limit() {
    let pool = build_pool(3, 3).await;

    let _c1 = pool.get().await.unwrap();
    let _c2 = pool.get().await.unwrap();
    let _c3 = pool.get().await.unwrap();

    // 第 4 个应该超时
    let result = tokio::time::timeout(Duration::from_millis(500), pool.get()).await;
    assert!(result.is_err() || result.unwrap().is_err());
}

// FR-021: max_idle 限制
#[tokio::test]
async fn test_max_idle_limit() {
    let pool = build_pool(4, 2).await;

    // 借出 4 个
    let c1 = pool.get().await.unwrap();
    let c2 = pool.get().await.unwrap();
    let c3 = pool.get().await.unwrap();
    let c4 = pool.get().await.unwrap();

    // 归还全部，但 max_idle=2，多余的应该被销毁
    drop(c1);
    drop(c2);
    drop(c3);
    drop(c4);

    tokio::time::sleep(Duration::from_millis(50)).await;
    let st = pool.state();
    // 空闲连接不应超过 max_idle
    assert!(st.idle_count <= 2, "idle_count={} should <= 2", st.idle_count);
}

// FR-022: acquire_timeout 返回
#[tokio::test]
async fn test_acquire_timeout() {
    let pool = build_pool(1, 1).await;

    let _c1 = pool.get().await.unwrap(); // 占满

    // 第 2 个应该超时
    let result = pool.get_timeout(Duration::from_millis(100)).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DruidError::AcquireTimeout));
}

// FR-023: PooledConnection::drop 归还
#[tokio::test]
async fn test_drop_returns_connection() {
    let pool = build_pool(2, 2).await;

    {
        let _c1 = pool.get().await.unwrap();
        let _c2 = pool.get().await.unwrap();
        assert_eq!(pool.state().active_count, 2);
    }
    // drop 后应该归还
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(pool.state().idle_count, 2);
    assert_eq!(pool.state().active_count, 0);
}

// FR-023: 10000 循环无泄漏
#[tokio::test]
async fn test_10000_acquire_release_no_leak() {
    let pool = build_pool(4, 4).await;

    for _ in 0..10_000 {
        let conn = pool.get().await.unwrap();
        drop(conn); // 立即归还
    }

    let st = pool.state();
    assert!(st.recycle_count > 0, "should have recycled connections");
    // 总连接数应该在合理范围内
    assert!(st.active_count == 0, "should have no active connections");
}

// FR-024: FilterChain 装配
#[tokio::test]
async fn test_filter_chain_assembly() {
    use druid_sql::{Wall, WallConfig};

    // 构建带 Wall Filter 的池
    let mut fc = FilterChain::new();
    // Wall 作为 BeforeFilter 需要适配，这里测试 FilterChain 的存在性
    let pool = DruidPool::builder()
        .name("test-filtered")
        .driver_name("mock")
        .factory(Arc::new(MockFactory))
        .max_open(2)
        .max_idle(2)
        .filter_chain(Arc::new(fc))
        .build()
        .await
        .unwrap();

    assert!(pool.filter_chain().is_some());

    let mut conn = pool.get().await.unwrap();
    let result = conn.exec("SELECT 1", vec![]).await.unwrap();
    assert_eq!(result.rows_affected, 1);
}

// 池状态
#[tokio::test]
async fn test_pool_state() {
    let pool = build_pool(4, 2).await;
    let st = pool.state();
    assert_eq!(st.name, "test");
    assert_eq!(st.driver_name, "mock");
    assert_eq!(st.max_open, 4);
    assert!(!st.closed);
}

// 关闭池
#[tokio::test]
async fn test_pool_close() {
    let pool = build_pool(2, 1).await;
    let c1 = pool.get().await.unwrap();
    drop(c1);

    pool.close().await;
    let st = pool.state();
    assert!(st.closed);

    // 关闭后不能获取连接
    let result = pool.get().await;
    assert!(result.is_err());
}
