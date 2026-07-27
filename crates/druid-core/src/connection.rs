//! 对应 Java 类：java.sql.Connection
//!
//! 连接 trait 定义，是 druid-rust 所有横切层的拦截点。

use crate::error::DruidError;
use crate::value::Value;

/// 执行结果。
#[derive(Debug, Clone, Default)]
pub struct ExecResult {
    pub rows_affected: u64,
    pub last_insert_id: Option<i64>,
}

/// 行数据。
#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<Value>,
}

impl Row {
    pub fn new(values: Vec<Value>) -> Self { Self { values } }
    pub fn get(&self, index: usize) -> Option<&Value> { self.values.get(index) }
    pub fn len(&self) -> usize { self.values.len() }
    pub fn is_empty(&self) -> bool { self.values.is_empty() }
}

/// 连接 trait，替代 JDBC java.sql.Connection。
///
/// 所有适配器必须实现此 trait。Filter 链通过装饰器模式包装此 trait。
#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    /// 执行 SQL（INSERT/UPDATE/DELETE），返回受影响行数。
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError>;
    /// 执行查询，返回多行结果。
    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError>;
    /// 开始事务。
    async fn begin(&mut self) -> Result<(), DruidError>;
    /// 提交事务。
    async fn commit(&mut self) -> Result<(), DruidError>;
    /// 回滚事务。
    async fn rollback(&mut self) -> Result<(), DruidError>;
    /// 验证连接是否存活。
    async fn ping(&mut self) -> Result<(), DruidError>;
    /// 关闭连接。
    async fn close(&mut self) -> Result<(), DruidError>;
    /// 返回驱动名称。
    fn driver_name(&self) -> &str { "" }
    /// 返回连接是否已关闭。
    fn is_closed(&self) -> bool { false }
}
