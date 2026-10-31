//! 整仓验收：连接池借用/归还与 LIFO 复用。
mod support;

use druid::pool::DruidPool;
use std::time::Duration;
use support::{connection_id, MockFactory};

#[tokio::test]
async fn pool_reuses_the_last_returned_holder_and_closes_cleanly() {
    let pool = DruidPool::builder()
        .name("acceptance-pool")
        .driver_name("mock")
        .factory(MockFactory::shared())
        .max_open(2)
        .max_idle(2)
        .min_idle(0)
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await
        .expect("pool builds");

    let mut first = pool.get().await.expect("first borrow");
    let mut second = pool.get().await.expect("second borrow");
    let first_id = connection_id(&mut first).await;
    let second_id = connection_id(&mut second).await;
    assert_ne!(first_id, second_id);

    druid::core::Connection::close(&mut first)
        .await
        .expect("first returns");
    druid::core::Connection::close(&mut second)
        .await
        .expect("second returns");

    let mut third = pool.get().await.expect("third borrow");
    assert_eq!(
        connection_id(&mut third).await,
        second_id,
        "Java pollLast is LIFO"
    );
    druid::core::Connection::close(&mut third)
        .await
        .expect("third returns");

    pool.close().await;
    assert_eq!(pool.state().active_count, 0);
    assert_eq!(pool.state().idle_count, 0);
}
