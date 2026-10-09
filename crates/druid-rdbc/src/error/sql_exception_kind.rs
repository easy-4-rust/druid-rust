//! Java SQL 异常层级对应的标准分类。

/// SQL 异常分类值，保留具体类型优先于 SQLState 的识别策略。
/// 对应 Java: `java.sql.SQLException` 及其标准子类型。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SqlExceptionKind {
    /// 可重试的 SQL 异常。对应 Java: `java.sql.SQLTransientException`。
    Transient,
    /// 可重试的连接异常。对应 Java: `java.sql.SQLTransientConnectionException`。
    TransientConnection,
    /// 不可直接重试的 SQL 异常。对应 Java: `java.sql.SQLNonTransientException`。
    NonTransient,
    /// 不可直接重试的连接异常。对应 Java: `java.sql.SQLNonTransientConnectionException`。
    NonTransientConnection,
    /// 恢复连接后可重新执行的异常。对应 Java: `java.sql.SQLRecoverableException`。
    Recoverable,
    /// SQL 数据异常。对应 Java: `java.sql.SQLDataException`。
    Data,
    /// 完整性约束异常。对应 Java: `java.sql.SQLIntegrityConstraintViolationException`。
    IntegrityConstraintViolation,
    /// 授权规范异常。对应 Java: `java.sql.SQLInvalidAuthorizationSpecException`。
    InvalidAuthorizationSpec,
    /// SQL 语法异常。对应 Java: `java.sql.SQLSyntaxErrorException`。
    Syntax,
    /// 事务回滚异常。对应 Java: `java.sql.SQLTransactionRollbackException`。
    TransactionRollback,
    /// 不支持的 SQL 功能异常。对应 Java: `java.sql.SQLFeatureNotSupportedException`。
    FeatureNotSupported,
    /// SQL 操作超时。对应 Java: `java.sql.SQLTimeoutException`。
    Timeout,
    /// 未细分的 SQL 异常。对应 Java: `java.sql.SQLException`。
    General,
}

impl SqlExceptionKind {
    /// 授权异常分类的历史名称，与标准 InvalidAuthorizationSpec 是同一个值。
    /// 对应 Java: `java.sql.SQLInvalidAuthorizationSpecException`。
    #[allow(
        non_upper_case_globals,
        reason = "保留已有公开枚举名称的调用与匹配兼容性"
    )]
    pub const InvalidAuthorization: Self = Self::InvalidAuthorizationSpec;
}

impl std::fmt::Debug for SqlExceptionKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 标准命名修复不改变既有错误日志中的分类表示。
        formatter.write_str(match self {
            Self::Transient => "Transient",
            Self::TransientConnection => "TransientConnection",
            Self::NonTransient => "NonTransient",
            Self::NonTransientConnection => "NonTransientConnection",
            Self::Recoverable => "Recoverable",
            Self::Data => "Data",
            Self::IntegrityConstraintViolation => "IntegrityConstraintViolation",
            Self::InvalidAuthorizationSpec => "InvalidAuthorization",
            Self::Syntax => "Syntax",
            Self::TransactionRollback => "TransactionRollback",
            Self::FeatureNotSupported => "FeatureNotSupported",
            Self::Timeout => "Timeout",
            Self::General => "General",
        })
    }
}
