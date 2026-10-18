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

#[tokio::test]
async fn interrupted_and_timed_out_waiters_leave_pool_reusable() {
    // 对应 Java 1.2.28 DruidDataSourceTest_maxWait#test_maxWait：唯一连接被占用时，
    // 两个等待者分别中断和超时，均不得取得连接；之后池仍可正常借还。
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("create SQLite factory");
    let pool = Arc::new(
        DruidPool::builder()
            .name("interrupted-and-timed-out-waiters")
            .driver_name("sqlite")
            .factory(Arc::new(factory))
            .max_open(1)
            .max_wait_thread_count(Some(2))
            .acquire_timeout(Duration::from_millis(250))
            .build()
            .await
            .expect("build pool"),
    );
    let held = pool.get().await.expect("occupy the only connection");
    let interrupted_pool = Arc::clone(&pool);
    let interrupted = tokio::spawn(async move { interrupted_pool.get().await });
    let timed_out_pool = Arc::clone(&pool);
    let timed_out = tokio::spawn(async move { timed_out_pool.get().await });

    tokio::time::timeout(Duration::from_secs(2), async {
        while pool.state().wait_count != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("both borrowers entered the wait queue");
    interrupted.abort();
    assert!(interrupted
        .await
        .expect_err("interrupted waiter must not borrow")
        .is_cancelled());
    let error = tokio::time::timeout(Duration::from_secs(2), timed_out)
        .await
        .expect("second waiter must terminate")
        .expect("second task must not panic")
        .expect_err("second waiter must time out");
    assert!(matches!(error, DruidError::GetConnectionTimeout { .. }));
    assert_eq!(pool.state().wait_count, 0);

    drop(held);
    let reused = tokio::time::timeout(Duration::from_secs(2), pool.get())
        .await
        .expect("pool must remain responsive")
        .expect("pool must return the released connection");
    drop(reused);
    pool.close().await;
}
