//! 池寿命分支测试：直接控制空闲 holder 的年龄，避免依赖毫秒级调度。

#[path = "lifetime_tests/lifetime_test_connection.rs"]
mod lifetime_test_connection;
#[path = "lifetime_tests/lifetime_test_factory.rs"]
mod lifetime_test_factory;

use self::lifetime_test_factory::LifetimeTestFactory;
use super::DruidPool;
use crate::core::Connection;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

async fn task6_lifetime_pool(lifetime: Duration) -> (DruidPool, Arc<LifetimeTestFactory>) {
    let factory = Arc::new(LifetimeTestFactory::default());
    let pool = DruidPool::builder()
        .name("lifetime-skip")
        .driver_name("fault-test")
        .factory(factory.clone())
        .max_open(4)
        .max_idle(4)
        .acquire_timeout(Duration::from_millis(200))
        .max_lifetime(lifetime)
        .min_idle(0)
        .build()
        .await
        .unwrap();
    pool.init().await.unwrap();
    (pool, factory)
}

#[tokio::test]
async fn task6_pool_deep_branch_coverage_test_borrow_skips_lifetime_expired_idle() {
    let (pool, factory) = task6_lifetime_pool(Duration::from_millis(10)).await;
    // 用正式建连路径创建真实 holder；只在测试夹具中建立待借出的空闲状态。
    // 不依赖“get + close 必须在 10ms 内完成”的调度假设。
    let holder = pool.inner.create_connection().await.unwrap();
    let expired_id = holder.connection_id();
    pool.inner.idle.lock().push_back(holder);
    pool.inner.record_pooling_count(1);
    assert_eq!(pool.state().idle_count, 1);
    assert_eq!(pool.state().active_count, 0);
    {
        let mut idle = pool.inner.idle.lock();
        let holder = idle.front_mut().unwrap();
        holder.created_at = Instant::now() - Duration::from_millis(30);
        assert!(holder.connection_age() >= Duration::from_millis(10));
    }
    let mut conn = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    assert_ne!(
        conn.connection_holder().unwrap().connection_id(),
        expired_id
    );
    assert_eq!(factory.creates.load(Ordering::SeqCst), 2);
    assert_eq!(pool.state().destroy_count, 1);
    conn.close().await.unwrap();
    let conn2 = pool.get().await.unwrap();
    assert_eq!(pool.state().active_count, 1);
    drop(conn2);
    pool.close().await;
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(
        factory.closes.load(Ordering::SeqCst),
        factory.creates.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn task6_lifetime_return_discards_expired_holder_exactly_once() {
    let (pool, factory) = task6_lifetime_pool(Duration::from_millis(10)).await;
    let mut conn = pool.get().await.unwrap();
    let holder = conn.connection_holder_mut().unwrap();
    let expired_id = holder.connection_id();
    holder.created_at = Instant::now() - Duration::from_secs(1);
    conn.close().await.unwrap();
    let state = pool.state();
    assert_eq!(state.active_count, 0);
    assert_eq!(state.idle_count, 0);
    assert_eq!(state.discard_count, 1);
    assert_eq!(state.destroy_count, 1);
    assert_eq!(state.close_count, 1);
    assert_eq!(state.recycle_count, 0);
    assert_eq!(factory.closes.load(Ordering::SeqCst), 1);
    let next = pool.get().await.unwrap();
    assert_ne!(
        next.connection_holder().unwrap().connection_id(),
        expired_id
    );
    drop(next);
    pool.close().await;
    assert_eq!(
        factory.closes.load(Ordering::SeqCst),
        factory.creates.load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn task6_lifetime_return_reuses_nonexpired_holder() {
    // 独立复用控制；迁移的超龄借出测试仍保留原 10ms 阈值。
    let (pool, factory) = task6_lifetime_pool(Duration::from_secs(3600)).await;
    let mut conn = pool.get().await.unwrap();
    let id = conn.connection_holder().unwrap().connection_id();
    conn.close().await.unwrap();
    assert_eq!(pool.state().idle_count, 1);
    assert_eq!(pool.state().recycle_count, 1);
    assert_eq!(pool.state().destroy_count, 0);
    let next = pool.get().await.unwrap();
    assert_eq!(next.connection_holder().unwrap().connection_id(), id);
    assert_eq!(factory.creates.load(Ordering::SeqCst), 1);
    drop(next);
    pool.close().await;
    assert_eq!(factory.closes.load(Ordering::SeqCst), 1);
}
