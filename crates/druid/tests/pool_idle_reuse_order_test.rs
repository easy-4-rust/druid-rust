//! SEM-POOL-001：idle holder 复用顺序必须对齐 Java `DruidDataSource#pollLast`（LIFO）。
//!
//! Java 语义基线：`com.alibaba.druid.pool.DruidDataSource#pollLast` 从 holder 数组尾部
//! 取连接，`putLast` 追加到尾部，因此“后归还者先复用”。本测试用可观测的连接序号验证
//! 该顺序，防止实现退化为 FIFO（FIFO 会改变热连接复用率、testWhileIdle 的校验对象
//! 以及 shrink 的驱逐次序）。

use druid::core::*;
use druid::pool::DruidPool;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 携带自增序号的 mock 物理连接；序号通过 `exec` 的 last_insert_id 暴露给测试。
struct OrderedConnection {
    id: u64,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(OrderedConnection => Connection);

#[async_trait::async_trait]
impl Connection for OrderedConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: Some(i64::try_from(self.id).expect("connection id fits i64")),
            row_count: None,
        })
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("ordered-mock".to_owned())
    }
}

/// 每次 create 递增序号，模拟不同的物理连接。
struct OrderedFactory {
    next: AtomicU64,
}

#[async_trait::async_trait]
impl ConnectionFactory for OrderedFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Box::new(OrderedConnection { id, closed: false }))
    }

    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        conn.ping().await
    }
}

async fn exec_connection_id(conn: &mut DruidPooledConnection) -> i64 {
    conn.exec("select 1", Vec::new())
        .await
        .expect("mock exec succeeds")
        .last_insert_id
        .expect("mock exec exposes connection id")
}

#[tokio::test]
async fn idle_holder_is_reused_in_last_returned_first_order() {
    let pool = DruidPool::builder()
        .name("lifo-order")
        .driver_name("ordered-mock")
        .factory(Arc::new(OrderedFactory {
            next: AtomicU64::new(0),
        }))
        .max_open(2)
        .max_idle(2)
        .min_idle(0)
        .acquire_timeout(Duration::from_secs(2))
        .build()
        .await
        .expect("pool builds");

    let mut first = pool.get().await.expect("first borrow");
    let mut second = pool.get().await.expect("second borrow");
    let first_id = exec_connection_id(&mut first).await;
    let second_id = exec_connection_id(&mut second).await;
    assert_ne!(
        first_id, second_id,
        "two borrows must use distinct physical connections"
    );

    // 归还顺序：first 先、second 后。Java pollLast 期望下一次借到 second。
    Connection::close(&mut first).await.expect("first returns");
    Connection::close(&mut second)
        .await
        .expect("second returns");

    let mut third = pool.get().await.expect("third borrow");
    let third_id = exec_connection_id(&mut third).await;
    assert_eq!(
        third_id, second_id,
        "Java DruidDataSource#pollLast reuses the last returned holder (LIFO)"
    );

    Connection::close(&mut third).await.expect("third returns");
    pool.close().await;
}
