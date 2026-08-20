//! `ResultSet#getStatement()` 动态平台对象。
//!
//! 对应 Java：`java.sql.ResultSet#getStatement()` 返回的 `Statement`，
//! 运行时对象可能是普通、Prepared 或 Callable Statement。

use super::{
    DruidPooledCallableStatementHandle, DruidPooledPreparedStatementHandle, DruidPooledStatement,
};

/// 保留 `ResultSet#getStatement()` 三种动态逻辑身份的拥有型句柄。
///
/// Clone 仅复制共享句柄；关闭状态、异常计数、缓存归还所有权和物理语句均与
/// 创建结果集的原逻辑 Statement 共享，不会生成字段快照或第二个逻辑对象。
/// 对应 Java：`java.sql.ResultSet#getStatement()` 的 `java.sql.Statement` 返回对象。
#[derive(Clone)]
pub enum ResultSetStatement {
    /// 普通池化 Statement。
    Statement(DruidPooledStatement),
    /// PreparedStatement 共享身份句柄。
    Prepared(DruidPooledPreparedStatementHandle),
    /// CallableStatement 共享身份句柄。
    Callable(DruidPooledCallableStatementHandle),
}

impl ResultSetStatement {
    /// 返回三种动态类型共同的池化 Statement 视图。
    pub fn pooled_statement(&self) -> &DruidPooledStatement {
        match self {
            Self::Statement(statement) => statement,
            Self::Prepared(statement) => statement.pooled_statement(),
            Self::Callable(statement) => statement.pooled_statement(),
        }
    }

    /// 返回 PreparedStatement 身份；CallableStatement 也继承该身份。
    pub fn prepared_statement(&self) -> Option<&DruidPooledPreparedStatementHandle> {
        match self {
            Self::Statement(_) => None,
            Self::Prepared(statement) => Some(statement),
            Self::Callable(statement) => Some(statement.prepared_statement()),
        }
    }

    /// 返回 CallableStatement 身份。
    pub fn callable_statement(&self) -> Option<&DruidPooledCallableStatementHandle> {
        match self {
            Self::Callable(statement) => Some(statement),
            Self::Statement(_) | Self::Prepared(_) => None,
        }
    }

    /// 返回逻辑 Statement 是否已经关闭。
    pub fn is_closed(&self) -> bool {
        match self {
            Self::Statement(statement) => statement.is_closed(),
            Self::Prepared(statement) => statement.is_closed(),
            Self::Callable(statement) => statement.is_closed(),
        }
    }
}
