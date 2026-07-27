//! 对应 Java 类：java.sql.Connection
//! 来源文件：core/src/main/java/com/alibaba/druid/proxy/jdbc/ConnectionProxyImpl.java
//!
//! 连接 trait 定义，替代 JDBC java.sql.Connection 的完整事务语义。

use crate::error::DruidError;
use crate::value::Value;
use std::time::Duration;

/// 执行结果，对应 Druid Java 的 Statement 执行返回值。
#[derive(Debug, Clone, Default)]
pub struct ExecResult {
    /// 受影响行数（对应 getUpdateCount()）
    pub rows_affected: u64,
    /// 最后插入的 ID（对应 getGeneratedKeys）
    pub last_insert_id: Option<i64>,
    /// 返回行数（对 ResultSet 的 getRowCount）
    pub row_count: Option<u64>,
}

/// 行数据，对应 JDBC ResultSet 的一行。
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

/// Savepoint 句柄，对应 JDBC Savepoint。
///
/// DruidJava 的 `DruidPooledConnection` 实现了 `setSavepoint()`
/// 和 `setSavepoint(String)` 两个重载。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Savepoint {
    /// Savepoint ID（自增）
    pub id: u64,
    /// Savepoint 名称（可选，对应 setSavepoint(String name)）
    pub name: Option<String>,
}

/// 连接状态，对应 JDBC Connection 的 autoCommit / readOnly / transactionIsolation。
#[derive(Debug, Clone)]
pub struct ConnState {
    /// 自动提交（对应 Connection.getAutoCommit()）
    pub auto_commit: bool,
    /// 只读（对应 Connection.getReadOnly()）
    pub read_only: bool,
    /// 事务隔离级别（对应 Connection.getTransactionIsolation()）
    ///
    /// JDBC 常量：
    /// - `TRANSACTION_READ_UNCOMMITTED` = 1
    /// - `TRANSACTION_READ_COMMITTED` = 2
    /// - `TRANSACTION_REPEATABLE_READ` = 4
    /// - `TRANSACTION_SERIALIZABLE` = 8
    pub transaction_isolation: u8,
    /// catalog（对应 Connection.getCatalog()）
    pub catalog: Option<String>,
    /// schema（对应 Connection.getSchema()）
    pub schema: Option<String>,
}

impl Default for ConnState {
    fn default() -> Self {
        Self {
            auto_commit: true,
            read_only: false,
            transaction_isolation: 2, // READ_COMMITTED
            catalog: None,
            schema: None,
        }
    }
}

/// 连接 trait，替代 JDBC java.sql.Connection。
///
/// 对应 DruidJava `DruidPooledConnection` 的核心方法，
/// 包含事务管理、连接控制、元数据查询等完整 JDBC 语义。
///
/// 所有适配器（rbdc / sqlx-deadpool / sqlx-bb8）必须实现此 trait。
/// Filter 链通过装饰器模式包装此 trait 的实现。
#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    // ── SQL 执行 ────────────────────────────────────────────────
    /// 执行 SQL（INSERT/UPDATE/DELETE），返回受影响行数。
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError>;

    /// 执行查询，返回多行结果。
    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError>;

    // ── 事务管理 ────────────────────────────────────────────────
    /// 开始事务（对应 Connection.beginTransaction()）。
    async fn begin(&mut self) -> Result<(), DruidError>;

    /// 提交事务（对应 Connection.commit()）。
    async fn commit(&mut self) -> Result<(), DruidError>;

    /// 回滚事务（对应 Connection.rollback()）。
    async fn rollback(&mut self) -> Result<(), DruidError>;

    /// 回滚到指定 savepoint（对应 Connection.rollback(Savepoint)）。
    async fn rollback_to(&mut self, _savepoint: &Savepoint) -> Result<(), DruidError> {
        Err(DruidError::Other("savepoint rollback not supported".into()))
    }

    /// 创建 savepoint（对应 Connection.setSavepoint()）。
    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        Err(DruidError::Other("savepoints not supported".into()))
    }

    /// 创建命名 savepoint（对应 Connection.setSavepoint(String)）。
    async fn set_savepoint_named(&mut self, _name: &str) -> Result<Savepoint, DruidError> {
        Err(DruidError::Other("savepoints not supported".into()))
    }

    /// 释放 savepoint（对应 Connection.releaseSavepoint(Savepoint)）。
    async fn release_savepoint(&mut self, _savepoint: &Savepoint) -> Result<(), DruidError> {
        Err(DruidError::Other("release savepoint not supported".into()))
    }

    /// 中止连接（对应 Connection.abort(Executor)）。
    ///
    /// 与 rollback 不同：abort 会强制关闭底层连接，
    /// 即使事务正在进行中。
    async fn abort(&mut self) -> Result<(), DruidError> {
        self.close().await
    }

    // ── 连接控制 ────────────────────────────────────────────────
    /// 验证连接是否存活（对应 Connection.isValid(int)）。
    async fn ping(&mut self) -> Result<(), DruidError>;

    /// 关闭连接（对应 Connection.close()）。
    async fn close(&mut self) -> Result<(), DruidError>;

    /// 返回连接是否已关闭（对应 Connection.isClosed()）。
    fn is_closed(&self) -> bool { false }

    // ── 属性查询 ────────────────────────────────────────────────
    /// 返回自动提交状态（对应 Connection.getAutoCommit()）。
    fn auto_commit(&self) -> bool { true }

    /// 设置自动提交（对应 Connection.setAutoCommit(boolean)）。
    async fn set_auto_commit(&mut self, _auto_commit: bool) -> Result<(), DruidError> { Ok(()) }

    /// 返回只读状态（对应 Connection.getReadOnly()）。
    fn read_only(&self) -> bool { false }

    /// 设置只读（对应 Connection.setReadOnly(boolean)）。
    async fn set_read_only(&mut self, _read_only: bool) -> Result<(), DruidError> { Ok(()) }

    /// 返回事务隔离级别（对应 Connection.getTransactionIsolation()）。
    fn transaction_isolation(&self) -> u8 { 2 } // READ_COMMITTED

    /// 设置事务隔离级别（对应 Connection.setTransactionIsolation(int)）。
    async fn set_transaction_isolation(&mut self, _level: u8) -> Result<(), DruidError> { Ok(()) }

    /// 返回 catalog（对应 Connection.getCatalog()）。
    fn catalog(&self) -> Option<&str> { None }

    /// 设置 catalog（对应 Connection.setCatalog(String)）。
    async fn set_catalog(&mut self, _catalog: &str) -> Result<(), DruidError> { Ok(()) }

    /// 返回 schema（对应 Connection.getSchema()）。
    fn schema(&self) -> Option<&str> { None }

    /// 设置 schema（对应 Connection.setSchema(String)）。
    async fn set_schema(&mut self, _schema: &str) -> Result<(), DruidError> { Ok(()) }

    // ── 驱动信息 ────────────────────────────────────────────────
    /// 返回驱动名称（如 "postgres"、"mysql"）。
    fn driver_name(&self) -> &str { "" }
}

// ── 扩展方法（V2+ 阶段）───────────────────────────────────────

/// Statement 类型，替代 JDBC Statement/PreparedStatement/CallableStatement。
///
/// DruidJava 中 Statement/PreparedStatement/CallableStatement 是独立类型，
/// Rust 中合并为一个 trait，通过 variant 区分。
#[derive(Debug, Clone)]
pub enum StatementType {
    /// Statement（对应 Connection.createStatement()）
    Statement,
    /// PreparedStatement（对应 Connection.prepareStatement(sql)）
    PreparedStatement(String),
    /// CallableStatement（对应 Connection.prepareCall(sql)）
    CallableStatement(String),
}

/// 数据源元数据（简化版，对应 DatabaseMetaData）。
#[derive(Debug, Clone, Default)]
pub struct MetaData {
    /// 数据库产品名称
    pub database_product_name: String,
    /// 数据库版本
    pub database_product_version: String,
    /// 驱动名称
    pub driver_name: String,
    /// 驱动版本
    pub driver_version: String,
    /// 驱动主版本号
    pub driver_major_version: i32,
    /// 驱动次版本号
    pub driver_minor_version: i32,
}

/// 扩展连接方法（V2+ 实现）。
///
/// 这些方法对应 DruidJava Connection 中的 Statement 创建和元数据查询。
/// 在 V1 阶段提供默认实现，V2 由适配器完善。
#[async_trait::async_trait]
pub trait ConnectionExt: Connection {
    /// 创建 Statement（对应 Connection.createStatement()）。
    async fn create_statement(&mut self) -> Result<Box<dyn Connection>, DruidError> {
        // 默认实现：返回自身引用（非真正 Statement）
        Err(DruidError::Other("createStatement not implemented".into()))
    }

    /// 创建 PreparedStatement（对应 Connection.prepareStatement(sql)）。
    async fn prepare_statement(&mut self, sql: &str) -> Result<Box<dyn Connection>, DruidError> {
        let _ = sql;
        Err(DruidError::Other("prepareStatement not implemented".into()))
    }

    /// 创建 CallableStatement（对应 Connection.prepareCall(sql)）。
    async fn prepare_call(&mut self, sql: &str) -> Result<Box<dyn Connection>, DruidError> {
        let _ = sql;
        Err(DruidError::Other("prepareCall not implemented".into()))
    }

    /// 获取元数据（对应 Connection.getMetaData()）。
    fn get_meta_data(&self) -> Option<&MetaData> { None }

    /// 获取数据库产品名称（对应 Connection.getDatabaseProductName()）。
    fn get_database_product_name(&self) -> Option<&str> { None }

    /// 获取数据库版本（对应 Connection.getDatabaseProductVersion()）。
    fn get_database_product_version(&self) -> Option<&str> { None }

    /// 获取驱动主版本号（对应 Connection.getDriverMajorVersion()）。
    fn get_driver_major_version(&self) -> i32 { 0 }

    /// 获取驱动次版本号（对应 Connection.getDriverMinorVersion()）。
    fn get_driver_minor_version(&self) -> i32 { 0 }

    /// 获取保持性（对应 Connection.getHoldability()）。
    fn get_holdability(&self) -> i32 { 1 } // HOLD_CURSORS_OVER_COMMIT

    /// 设置保持性（对应 Connection.setHoldability(int)）。
    async fn set_holdability(&mut self, _holdability: i32) -> Result<(), DruidError> { Ok(()) }

    /// 设置客户端信息（对应 Connection.setClientInfo(String, String)）。
    async fn set_client_info(&mut self, _name: &str, _value: &str) -> Result<(), DruidError> { Ok(()) }

    /// 获取客户端信息（对应 Connection.getClientInfo(String)）。
    fn get_client_info(&self, _name: &str) -> Option<String> { None }

    /// 清除警告（对应 Connection.clearWarnings()）。
    async fn clear_warnings(&mut self) -> Result<(), DruidError> { Ok(()) }

    /// 原生 SQL（对应 Connection.nativeSQL(String)）。
    async fn native_sql(&self, sql: &str) -> Result<String, DruidError> { Ok(sql.to_string()) }

    /// 设置网络超时（对应 Connection.setNetworkTimeout(Executor, int)）。
    async fn set_network_timeout(&mut self, _timeout: Duration) -> Result<(), DruidError> { Ok(()) }

    /// 获取网络超时（对应 Connection.getNetworkTimeout()）。
    fn get_network_timeout(&self) -> i32 { 0 }

    /// 获取类型映射（对应 Connection.getTypeMap()）。
    fn get_type_map(&self) -> Option<std::collections::HashMap<String, String>> { None }

    /// 设置类型映射（对应 Connection.setTypeMap(Map)）。
    async fn set_type_map(&mut self, _map: std::collections::HashMap<String, String>) -> Result<(), DruidError> { Ok(()) }
}
