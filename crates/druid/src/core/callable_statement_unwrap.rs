//! `DruidPooledCallableStatement#unwrap(Class<T>)` 的 Rust 目标与返回值。

use super::{DruidPooledCallableStatement, PhysicalCallableStatement, PhysicalPreparedStatement};

/// Java `unwrap` 请求的接口身份。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallableStatementUnwrapTarget {
    /// `DruidPooledCallableStatement.class`。
    PooledCallableStatement,
    /// `CallableStatement.class`。
    CallableStatement,
    /// `PreparedStatement.class`。
    PreparedStatement,
    /// 交给上层 wrapper/驱动处理的其他 Java 接口名。
    Other(String),
}

/// `unwrap` 成功返回的对象身份。
pub enum CallableStatementUnwrapped<'a> {
    /// 当前池化 wrapper。
    Pooled(&'a DruidPooledCallableStatement),
    /// 原始物理 callable 句柄。
    Callable(&'a dyn PhysicalCallableStatement),
    /// 原始物理 prepared 句柄。
    Prepared(&'a dyn PhysicalPreparedStatement),
}

impl std::fmt::Debug for CallableStatementUnwrapped<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pooled(_) => formatter.write_str("PooledCallableStatement"),
            Self::Callable(_) => formatter.write_str("CallableStatement"),
            Self::Prepared(_) => formatter.write_str("PreparedStatement"),
        }
    }
}
