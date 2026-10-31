//! 整仓验收：DruidPooledConnection 满足 rdbc 标准 Connection 合同。
mod support;

use druid::core::Connection;
use druid::pool::DruidPool;
use std::time::Duration;
use support::{connection_id, MockFactory};

#[tokio::test]
async fn pooled_connection_implements_the_standard_connection_contract() {
    let pool = DruidPool::builder()
        .name("acceptance-standard")
        .driver_name("mock")
        .factory(MockFactory::shared())
        .max_open(1)
        .max_idle(1)
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await
        .expect("pool builds");

    let mut conn = pool.get().await.expect("borrow");
    assert!(!conn.is_closed().expect("is_closed"));
    assert_eq!(conn.driver_name().expect("driver name"), "mock");
    assert_eq!(connection_id(&mut conn).await, 1);
    assert_eq!(
        conn.fetch("select 1", Vec::new())
            .await
            .expect("fetch")
            .len(),
        1
    );

    conn.close().await.expect("close");
    assert!(conn.is_closed().expect("closed"));
    pool.close().await;
}
