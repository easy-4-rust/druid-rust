//! 首次入池捕获的标准连接状态，供每次租约结束后复位。

use super::DruidError;
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::error::{RdbcError, RdbcErrorKind};
use druid_rdbc::statement::ResultSetHoldability;

/// 驱动连接返回空闲队列前恢复的完整状态及真实支持标记。
/// 对应 Java: com.alibaba.druid.pool.DruidConnectionHolder#reset。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDefaults {
    auto_commit: bool,
    read_only: bool,
    holdability: ResultSetHoldability,
    transaction_isolation: TransactionIsolation,
    catalog: Option<String>,
    schema: Option<String>,
    auto_commit_supported: bool,
    read_only_supported: bool,
    holdability_supported: bool,
    transaction_isolation_supported: bool,
    catalog_supported: bool,
    schema_supported: bool,
    warnings_supported: bool,
}

impl Default for ConnectionDefaults {
    fn default() -> Self {
        Self {
            auto_commit: true,
            read_only: false,
            holdability: ResultSetHoldability::DriverValue(0),
            transaction_isolation: TransactionIsolation::READ_COMMITTED,
            catalog: None,
            schema: None,
            auto_commit_supported: false,
            read_only_supported: false,
            holdability_supported: false,
            transaction_isolation_supported: false,
            catalog_supported: false,
            schema_supported: false,
            warnings_supported: false,
        }
    }
}

impl ConnectionDefaults {
    /// 参数为实际入池连接；读取支持的完整状态，返回快照或真实驱动错误。
    /// 不支持与功能不支持错误仅标记该状态不支持，不伪造可复位能力。
    pub(crate) async fn capture_standard(
        connection: &mut dyn Connection,
    ) -> Result<Self, DruidError> {
        let auto_commit = optional_state(connection.auto_commit())?;
        let read_only = optional_state(connection.read_only())?;
        let holdability = optional_state(connection.holdability())?;
        let transaction_isolation = optional_state(connection.transaction_isolation())?;
        let catalog = optional_state(connection.catalog())?;
        let schema = optional_state(connection.schema())?;
        let warnings_supported = optional_state(connection.warnings().await)?.is_some();
        Ok(Self {
            auto_commit: auto_commit.unwrap_or(true),
            read_only: read_only.unwrap_or(false),
            holdability: holdability.unwrap_or(ResultSetHoldability::DriverValue(0)),
            transaction_isolation: transaction_isolation
                .unwrap_or(TransactionIsolation::READ_COMMITTED),
            catalog: catalog.clone().flatten(),
            schema: schema.clone().flatten(),
            auto_commit_supported: auto_commit.is_some(),
            read_only_supported: read_only.is_some(),
            holdability_supported: holdability.is_some(),
            transaction_isolation_supported: transaction_isolation.is_some(),
            catalog_supported: catalog.is_some(),
            schema_supported: schema.is_some(),
            warnings_supported,
        })
    }

    pub(crate) fn standard_auto_commit(&self) -> Option<bool> {
        self.auto_commit_supported.then_some(self.auto_commit)
    }

    pub(crate) fn standard_read_only(&self) -> Option<bool> {
        self.read_only_supported.then_some(self.read_only)
    }

    pub(crate) const fn standard_warnings_supported(&self) -> bool {
        self.warnings_supported
    }

    /// 无参数；返回捕获的自动提交值，对应 Java: Connection#getAutoCommit。
    #[must_use]
    pub const fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    /// 无参数；返回捕获的只读值，对应 Java: Connection#isReadOnly。
    #[must_use]
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// 无参数；返回捕获的原始游标保留代码，对应 Java: Connection#getHoldability。
    #[must_use]
    pub const fn holdability(&self) -> i32 {
        self.holdability.rdbc_code()
    }

    /// 无参数；返回无窄化的隔离级别原代码，对应 Java: Connection#getTransactionIsolation。
    #[must_use]
    pub const fn transaction_isolation(&self) -> i32 {
        self.transaction_isolation.jdbc_code()
    }

    pub(crate) fn catalog(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    pub(crate) fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
    }

    /// 参数为当前连接及隔离级别保留选项；返回支持状态是否偏离快照或驱动错误。
    pub(crate) fn standard_needs_reset(
        &self,
        connection: &dyn Connection,
        keep_transaction_isolation: bool,
    ) -> Result<bool, DruidError> {
        Ok(
            (self.read_only_supported && connection.read_only()? != self.read_only)
                || (self.holdability_supported
                    && connection.holdability()?.rdbc_code() != self.holdability.rdbc_code())
                || (self.transaction_isolation_supported
                    && !keep_transaction_isolation
                    && connection.transaction_isolation()? != self.transaction_isolation)
                || (self.auto_commit_supported && connection.auto_commit()? != self.auto_commit)
                || (self.catalog_supported && connection.catalog()?.as_deref() != self.catalog())
                || (self.schema_supported && connection.schema()?.as_deref() != self.schema()),
        )
    }

    /// 参数为当前连接、schema复位与隔离级别保留选项；按原顺序恢复状态。
    /// 返回成功或首次驱动错误，失败后不再调用后续setter；对应 Java: DruidConnectionHolder#reset。
    pub(crate) async fn reset_standard_state(
        &self,
        connection: &mut dyn Connection,
        reset_schema: bool,
        keep_transaction_isolation: bool,
    ) -> Result<(), DruidError> {
        if self.read_only_supported && connection.read_only()? != self.read_only {
            connection.set_read_only(self.read_only).await?;
        }
        // 保留 Java 四项状态的相对次序，隔离级别保留时不读取或修改该状态。
        if self.holdability_supported
            && connection.holdability()?.rdbc_code() != self.holdability.rdbc_code()
        {
            connection.set_holdability(self.holdability).await?;
        }
        if self.transaction_isolation_supported
            && !keep_transaction_isolation
            && connection.transaction_isolation()? != self.transaction_isolation
        {
            connection
                .set_transaction_isolation(self.transaction_isolation)
                .await?;
        }
        if self.catalog_supported && connection.catalog()?.as_deref() != self.catalog() {
            connection.set_catalog(self.catalog()).await?;
        }
        if reset_schema && self.schema_supported && connection.schema()?.as_deref() != self.schema()
        {
            connection.set_schema(self.schema()).await?;
        }
        if self.auto_commit_supported && connection.auto_commit()? != self.auto_commit {
            connection.set_auto_commit(self.auto_commit).await?;
        }
        Ok(())
    }
}

fn optional_state<T>(result: Result<T, RdbcError>) -> Result<Option<T>, DruidError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if matches!(
                error.kind(),
                RdbcErrorKind::UnsupportedOperation | RdbcErrorKind::SqlFeatureNotSupported
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod stateful_connection;

#[cfg(test)]
mod contract_lease;

#[cfg(test)]
mod capture_probe_connection;

#[cfg(test)]
mod complete_state_tests;

#[cfg(test)]
mod boundary_tests;
