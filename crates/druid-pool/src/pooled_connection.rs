//! 对应 Java 类：com.alibaba.druid.pool.DruidPooledConnection
//!
//! 池化连接，Drop 时自动归还。

use druid_core::{Connection, DruidError, ExecContext, ExecResult, FilterChain, Value, Row};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::pool_inner::PoolInner;

/// 池化连接，Drop 时自动归还到池。
pub struct DruidPoolConnection {
    conn: Option<Box<dyn Connection>>,
    id: u64,
    pool: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
}

impl std::fmt::Debug for DruidPoolConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DruidPoolConnection")
            .field("id", &self.id)
            .field("has_conn", &self.conn.is_some())
            .finish()
    }
}

impl DruidPoolConnection {
    pub(crate) fn new(conn: Box<dyn Connection>, id: u64, pool: Arc<PoolInner>, filter_chain: Option<Arc<FilterChain>>) -> Self {
        Self { conn: Some(conn), id, pool, filter_chain }
    }

    pub fn id(&self) -> u64 { self.id }

    pub async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        let start = Instant::now();
        if let Some(ref fc) = self.filter_chain {
            let mut ctx = ExecContext { sql, params: &params, data_source: "", start, fingerprint: None };
            fc.before_execute(&mut ctx).await?;
        }
        let result = self.conn.as_mut().expect("taken").exec(sql, params).await;
        let elapsed = start.elapsed();
        if let Some(ref fc) = self.filter_chain {
            let ctx = ExecContext { sql, params: &[], data_source: "", start, fingerprint: None };
            fc.after_execute(&ctx, &result, elapsed).await;
        }
        result
    }

    pub async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.conn.as_mut().expect("taken").fetch(sql, params).await
    }

    pub async fn ping(&mut self) -> Result<(), DruidError> {
        self.conn.as_mut().expect("taken").ping().await
    }

    pub fn driver_name(&self) -> &str {
        self.conn.as_ref().map(|c: &Box<dyn Connection>| c.driver_name()).unwrap_or("")
    }

    pub fn into_core(mut self) -> druid_core::PooledConnection {
        let conn = self.conn.take().expect("connection taken");
        druid_core::PooledConnection::new(conn, self.id, Box::new(|_, _| {}))
    }
}

impl Drop for DruidPoolConnection {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.take() {
            self.pool.return_connection(conn, self.id);
        }
    }
}
