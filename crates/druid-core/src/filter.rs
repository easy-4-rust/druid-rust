//! 对应 Java 类：com.alibaba.druid.filter.Filter
//! 来源文件：core/src/main/java/com/alibaba/druid/filter/Filter.java
//!
//! Filter trait 定义，对齐 DruidJava Filter 的核心 hook 方法。
//! DruidJava 有 100+ hook，这里保留最常用的 20+ 核心 hook，
//! 其余通过扩展 trait（ConnectionHook / StatementHook）覆盖。

use crate::error::DruidError;
use crate::value::Value;
use std::time::{Duration, Instant};

/// SQL 执行上下文，传递给 Filter 的 before/after 方法。
///
/// 对应 DruidJava Filter 方法的各种参数。
#[derive(Debug)]
pub struct ExecContext<'a> {
    /// SQL 文本
    pub sql: &'a str,
    /// SQL 参数
    pub params: &'a [Value],
    /// 数据源名称
    pub data_source: &'a str,
    /// 执行开始时间
    pub start: Instant,
    /// SQL 指纹（参数化后的哈希）
    pub fingerprint: Option<u64>,
}

/// 连接事件类型，对应 DruidJava 的 connection_* hook 系列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// 连接创建（对应 connection_connect）
    Connect,
    /// 连接关闭（对应 connection_close）
    Close,
    /// 设置自动提交（对应 connection_setAutoCommit）
    SetAutoCommit(bool),
    /// 获取自动提交（对应 connection_getAutoCommit）
    GetAutoCommit,
    /// 提交事务（对应 connection_commit）
    Commit,
    /// 回滚事务（对应 connection_rollback）
    Rollback,
    /// 设置只读（对应 connection_setReadOnly）
    SetReadOnly(bool),
    /// 获取只读（对应 connection_isReadOnly）
    GetReadOnly,
    /// 设置 catalog（对应 connection_setCatalog）
    SetCatalog(String),
    /// 获取 catalog（对应 connection_getCatalog）
    GetCatalog,
    /// 设置事务隔离级别（对应 connection_setTransactionIsolation）
    SetTransactionIsolation(u8),
    /// 获取事务隔离级别（对应 connection_getTransactionIsolation）
    GetTransactionIsolation,
    /// 清除警告（对应 connection_clearWarnings）
    ClearWarnings,
    /// 设置 schema（对应 connection_setSchema）
    SetSchema(String),
    /// 获取 schema（对应 connection_getSchema）
    GetSchema,
    /// 中止连接（对应 connection_abort）
    Abort,
    /// 验证连接（对应 connection_isValid）
    IsValid,
    /// 原生 SQL（对应 connection_nativeSQL）
    NativeSQL(String),
    /// 设置网络超时（对应 connection_setNetworkTimeout）
    SetNetworkTimeout(Duration),
    /// 获取网络超时（对应 connection_getNetworkTimeout）
    GetNetworkTimeout,
}

/// 语句事件类型，对应 DruidJava 的 statement_* / preparedStatement_* hook 系列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementEvent {
    /// 创建 Statement（对应 connection_createStatement）
    CreateStatement,
    /// 创建 PreparedStatement（对应 connection_prepareStatement）
    PrepareStatement(String),
    /// 创建 CallableStatement（对应 connection_prepareCall）
    PrepareCall(String),
    /// 执行语句（对应 statement_execute）
    Execute(String),
    /// 执行查询（对应 statement_executeQuery）
    ExecuteQuery(String),
    /// 执行更新（对应 statement_executeUpdate）
    ExecuteUpdate(String),
    /// 关闭语句（对应 statement_close）
    Close,
    /// 批量执行（对应 statement_executeBatch）
    ExecuteBatch,
}

/// 结果集事件类型，对应 DruidJava 的 resultSet_* hook 系列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultSetEvent {
    /// 移到下一行（对应 resultSet_next）
    Next,
    /// 关闭结果集（对应 resultSet_close）
    Close,
    /// 获取 String（对应 resultSet_getString）
    GetString,
    /// 获取 Boolean（对应 resultSet_getBoolean）
    GetBoolean,
    /// 获取 Int（对应 resultSet_getInt）
    GetInt,
    /// 移到首行（对应 resultSet_first）
    First,
    /// 移到末行（对应 resultSet_last）
    Last,
}

/// 前置 Filter trait。
///
/// 对应 DruidJava `Filter` 接口的 before 系列 hook。
/// 在 SQL 执行前调用，任一 Filter 返回 Err 则短路。
#[async_trait::async_trait]
pub trait BeforeFilter: Send + Sync {
    /// 返回 Filter 名称。
    fn name(&self) -> &str;

    /// 通用前置拦截（对应 Filter 的 before-execute 语义）。
    async fn before(&self, ctx: &mut ExecContext<'_>) -> Result<(), DruidError>;

    /// 连接事件拦截（对应 Filter 的 connection_* 系列 hook）。
    /// 返回 Ok(()) 放行，Err 短路。
    async fn on_connection_event(&self, _event: &ConnectionEvent) -> Result<(), DruidError> {
        Ok(()) // 默认放行
    }

    /// 语句事件拦截（对应 Filter 的 statement_* / preparedStatement_* 系列 hook）。
    async fn on_statement_event(&self, _event: &StatementEvent) -> Result<(), DruidError> {
        Ok(()) // 默认放行
    }

    /// 结果集事件拦截（对应 Filter 的 resultSet_* 系列 hook）。
    async fn on_result_set_event(&self, _event: &ResultSetEvent) -> Result<(), DruidError> {
        Ok(()) // 默认放行
    }

    /// 过滤器生命周期（对应 Filter.init()）。
    async fn init(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 过滤器销毁（对应 Filter.destroy()）。
    async fn destroy(&self) -> Result<(), DruidError> {
        Ok(())
    }
}

/// 后置 Filter trait。
///
/// 对应 DruidJava `Filter` 接口的 after-execute hook。
/// 即使 SQL 执行失败也会调用。
#[async_trait::async_trait]
pub trait AfterFilter: Send + Sync {
    /// 返回 Filter 名称。
    fn name(&self) -> &str;

    /// 后置拦截。
    async fn after(
        &self,
        ctx: &ExecContext<'_>,
        result: &Result<crate::connection::ExecResult, DruidError>,
        elapsed: Duration,
    );

    /// 连接关闭后置（对应 Filter.connection_close after）。
    async fn after_connection_close(&self) {}
}
