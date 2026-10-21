//! 整仓验收共享夹具：带序号的 mock 物理连接与工厂。

use druid::core::{Connection, ConnectionFactory, ExecResult, RdbcError, Row, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// 可观测的 mock 物理连接：序号通过 exec 的 last_insert_id 暴露。
pub struct MockConnection {
    id: u64,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(MockConnection => Connection);

#[async_trait::async_trait]
impl Connection for MockConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: Some(i64::try_from(self.id).expect("id fits i64")),
            row_count: None,
        })
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Ok(vec![Row::new(vec![Value::Int(1)])])
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
        Ok("mock".to_owned())
    }
}

/// 递增序号创建 mock 连接。
#[derive(Default)]
pub struct MockFactory {
    next: AtomicU64,
}

impl MockFactory {
    /// 创建带独立序号的工厂。
    pub fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait::async_trait]
impl ConnectionFactory for MockFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Box::new(MockConnection { id, closed: false }))
    }

    async fn validate(&self, conn: &mut Box<dyn Connection>) -> Result<(), RdbcError> {
        conn.ping().await
    }
}

/// 读取连接序号。
pub async fn connection_id(conn: &mut druid::core::DruidPooledConnection) -> i64 {
    Connection::exec(conn, "select 1", Vec::new())
        .await
        .expect("mock exec")
        .last_insert_id
        .expect("id present")
}
