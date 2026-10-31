//! 整仓验收：shrink 保持 Java `minIdle` 边界。
mod support;

use druid::pool::DruidPool;
use std::time::Duration;
use support::MockFactory;

#[tokio::test]
async fn shrink_keeps_exactly_min_idle() {
    let pool = DruidPool::builder()
        .name("acceptance-shrink")
        .driver_name("mock")
        .factory(MockFactory::shared())
        .max_open(4)
        .max_idle(4)
        .min_idle(2)
        .idle_timeout(Duration::from_millis(1))
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await
        .expect("pool builds");

    let mut held = Vec::new();
    for _ in 0..4 {
        held.push(pool.get().await.expect("borrow"));
    }
    for conn in &mut held {
        druid::core::Connection::close(conn).await.expect("return");
    }
    assert_eq!(pool.state().idle_count, 4);

    tokio::time::sleep(Duration::from_millis(5)).await;
    pool.shrink_with_options(true, false).await;
    assert_eq!(pool.state().idle_count, 2, "Java shrink keeps minIdle");
    pool.close().await;
}
