//! 等待队列达到配置上限时，下一次借用必须立即被拒绝。

use druid::core::DruidError;
use druid::pool::DruidPool;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn max_wait_thread_count_rejects_at_the_limit() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("create SQLite factory");
    let pool = Arc::new(
        DruidPool::builder()
            .name("wait-limit")
            .driver_name("sqlite")
            .factory(Arc::new(factory))
            .max_open(1)
            .max_wait_thread_count(Some(1))
            .acquire_timeout(Duration::from_secs(5))
            .build()
            .await
            .expect("build pool"),
    );
    let connection = pool.get().await.expect("occupy the only connection");

    let waiting_pool = Arc::clone(&pool);
    let waiter = tokio::spawn(async move { waiting_pool.get().await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while pool.state().wait_count != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("first request enters the wait queue");

    let rejected = tokio::time::timeout(Duration::from_millis(250), pool.get())
        .await
        .expect("second waiter must be rejected immediately")
        .expect_err("wait queue is already full");
    assert!(matches!(
        rejected,
        DruidError::MaxWaitThreadCountExceeded { max: 1, current: 1 }
    ));
    assert_eq!(pool.state().wait_count, 1);

    waiter.abort();
    let _ = waiter.await;
    drop(connection);
    pool.close().await;
}
