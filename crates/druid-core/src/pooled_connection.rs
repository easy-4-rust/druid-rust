//! 对应 Java 类：com.alibaba.druid.pool.DruidPooledConnection

use crate::connection::Connection;
use crate::error::DruidError;
use crate::value::Value;

/// 池化连接 RAII 句柄，Drop 时自动归还连接到池。
pub struct PooledConnection {
    conn: Option<Box<dyn Connection>>,
    id: u64,
    return_fn: Option<Box<dyn FnOnce(Box<dyn Connection>, u64) + Send>>,
}

impl PooledConnection {
    pub fn new(conn: Box<dyn Connection>, id: u64, return_fn: Box<dyn FnOnce(Box<dyn Connection>, u64) + Send>) -> Self {
        Self { conn: Some(conn), id, return_fn: Some(return_fn) }
    }

    pub fn id(&self) -> u64 { self.id }

    pub fn conn_mut(&mut self) -> Option<&mut (dyn Connection + 'static)> {
        self.conn.as_deref_mut()
    }

    pub fn take(mut self) -> Box<dyn Connection> {
        self.conn.take().expect("connection already taken")
    }

    pub fn recycle(mut self) {
        if let (Some(conn), Some(return_fn)) = (self.conn.take(), self.return_fn.take()) {
            return_fn(conn, self.id);
        }
    }

    pub async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<crate::connection::ExecResult, DruidError> {
        self.conn.as_mut().expect("connection taken").exec(sql, params).await
    }

    pub async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<crate::connection::Row>, DruidError> {
        self.conn.as_mut().expect("connection taken").fetch(sql, params).await
    }

    pub async fn begin(&mut self) -> Result<(), DruidError> { self.conn.as_mut().expect("taken").begin().await }
    pub async fn commit(&mut self) -> Result<(), DruidError> { self.conn.as_mut().expect("taken").commit().await }
    pub async fn rollback(&mut self) -> Result<(), DruidError> { self.conn.as_mut().expect("taken").rollback().await }
    pub async fn ping(&mut self) -> Result<(), DruidError> { self.conn.as_mut().expect("taken").ping().await }
    pub fn driver_name(&self) -> &str { self.conn.as_ref().map(|c| c.driver_name()).unwrap_or("") }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let (Some(conn), Some(return_fn)) = (self.conn.take(), self.return_fn.take()) {
            return_fn(conn, self.id);
        }
    }
}
