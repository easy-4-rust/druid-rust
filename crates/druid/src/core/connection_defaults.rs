//! 物理连接默认状态快照。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidConnectionHolder` 中的
//! `defaultReadOnly`、`defaultHoldability`、`defaultTransactionIsolation`
//! 与 `defaultAutoCommit` 字段，以及 `DruidConnectionHolder#reset()`。

use super::{DruidError, PhysicalConnection};
use druid_rdbc::connection::Connection as StandardConnection;
use druid_rdbc::error::{RdbcError, RdbcErrorKind};

/// 物理连接进入连接池时的默认状态。
///
/// 每个物理连接只在首次进入池时捕获一次默认值。归还连接时按照 Java
/// `DruidConnectionHolder#reset()` 的顺序恢复状态，防止前一个借用者的事务
/// 或连接属性泄漏给下一个借用者。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDefaults {
    auto_commit: bool,
    read_only: bool,
    holdability: i32,
    transaction_isolation: u8,
    catalog: Option<String>,
    schema: Option<String>,
    standard_auto_commit_supported: bool,
    standard_read_only_supported: bool,
    standard_catalog_supported: bool,
    standard_schema_supported: bool,
    standard_warnings_supported: bool,
}

impl Default for ConnectionDefaults {
    fn default() -> Self {
        Self {
            auto_commit: true,
            read_only: false,
            holdability: 0,
            transaction_isolation: 2,
            catalog: None,
            schema: None,
            standard_auto_commit_supported: false,
            standard_read_only_supported: false,
            standard_catalog_supported: false,
            standard_schema_supported: false,
            standard_warnings_supported: false,
        }
    }
}

impl ConnectionDefaults {
    /// 从物理连接捕获默认状态。
    ///
    /// 对应 Java：`DruidConnectionHolder` 构造函数。
    ///
    /// # 参数
    /// - `connection`：刚完成数据源默认配置初始化的物理连接。
    pub fn capture(connection: &dyn PhysicalConnection) -> Self {
        Self {
            auto_commit: connection.auto_commit(),
            read_only: connection.read_only(),
            holdability: connection.holdability(),
            transaction_isolation: connection.transaction_isolation(),
            catalog: connection.catalog().map(ToOwned::to_owned),
            schema: connection.schema().map(ToOwned::to_owned),
            standard_auto_commit_supported: false,
            standard_read_only_supported: false,
            standard_catalog_supported: false,
            standard_schema_supported: false,
            standard_warnings_supported: false,
        }
    }

    /// 从 canonical standard connection 捕获 Druid 回收所需默认状态。
    pub(crate) async fn capture_standard(
        connection: &mut dyn StandardConnection,
    ) -> Result<Self, DruidError> {
        let auto_commit = optional_standard_state(connection.auto_commit())?;
        let read_only = optional_standard_state(connection.read_only())?;
        let catalog = optional_standard_state(connection.catalog())?;
        let schema = optional_standard_state(connection.schema())?;
        let warnings_supported = optional_standard_state(connection.warnings().await)?.is_some();
        Ok(Self {
            auto_commit: auto_commit.unwrap_or(true),
            read_only: read_only.unwrap_or(false),
            holdability: 0,
            transaction_isolation: 2,
            catalog: catalog.clone().flatten(),
            schema: schema.clone().flatten(),
            standard_auto_commit_supported: auto_commit.is_some(),
            standard_read_only_supported: read_only.is_some(),
            standard_catalog_supported: catalog.is_some(),
            standard_schema_supported: schema.is_some(),
            standard_warnings_supported: warnings_supported,
        })
    }

    /// 返回 standard auto-commit 是否可观察及其首次值。
    pub(crate) fn standard_auto_commit(&self) -> Option<bool> {
        self.standard_auto_commit_supported
            .then_some(self.auto_commit)
    }

    /// 返回 standard read-only 是否可观察及其首次值。
    pub(crate) fn standard_read_only(&self) -> Option<bool> {
        self.standard_read_only_supported.then_some(self.read_only)
    }

    /// 返回 standard warning chain 是否可观察/清理。
    pub(crate) fn standard_warnings_supported(&self) -> bool {
        self.standard_warnings_supported
    }

    /// 返回默认自动提交状态。
    pub fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    /// 返回默认只读状态。
    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// 返回默认 `ResultSet` 保持性。
    pub fn holdability(&self) -> i32 {
        self.holdability
    }

    /// 返回默认事务隔离级别。
    pub fn transaction_isolation(&self) -> u8 {
        self.transaction_isolation
    }

    /// 返回首次进入池时的 catalog。
    pub(crate) fn catalog(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    /// 返回首次进入池时的 schema。
    pub(crate) fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
    }

    /// 判断 canonical standard connection 是否需要异步回收复位。
    pub(crate) fn standard_needs_reset(
        &self,
        connection: &dyn StandardConnection,
    ) -> Result<bool, DruidError> {
        Ok(
            (self.standard_read_only_supported && connection.read_only()? != self.read_only)
                || (self.standard_auto_commit_supported
                    && connection.auto_commit()? != self.auto_commit)
                || (self.standard_catalog_supported
                    && connection.catalog()?.as_deref() != self.catalog())
                || (self.standard_schema_supported
                    && connection.schema()?.as_deref() != self.schema()),
        )
    }

    /// 按 Druid holder 顺序恢复 canonical standard connection 状态。
    pub(crate) async fn reset_standard_state(
        &self,
        connection: &mut dyn StandardConnection,
    ) -> Result<(), DruidError> {
        if self.standard_read_only_supported && connection.read_only()? != self.read_only {
            connection.set_read_only(self.read_only).await?;
        }
        if self.standard_catalog_supported && connection.catalog()?.as_deref() != self.catalog() {
            connection.set_catalog(self.catalog()).await?;
        }
        if self.standard_schema_supported && connection.schema()?.as_deref() != self.schema() {
            connection.set_schema(self.schema()).await?;
        }
        if self.standard_auto_commit_supported && connection.auto_commit()? != self.auto_commit {
            connection.set_auto_commit(self.auto_commit).await?;
        }
        Ok(())
    }

    /// 判断连接是否需要异步复位。
    ///
    /// `keep_underlying_transaction_isolation` 对应 Java 数据源的同名配置；
    /// 为 `true` 时不比较事务隔离级别。
    pub fn needs_reset(
        &self,
        connection: &dyn PhysicalConnection,
        keep_underlying_transaction_isolation: bool,
    ) -> bool {
        let capabilities = connection.capabilities();
        (capabilities.read_only && connection.read_only() != self.read_only)
            || (capabilities.holdability && connection.holdability() != self.holdability)
            || (!keep_underlying_transaction_isolation
                && capabilities.transaction_isolation
                && connection.transaction_isolation() != self.transaction_isolation)
            || (capabilities.auto_commit && connection.auto_commit() != self.auto_commit)
    }

    /// 按 Java `DruidConnectionHolder#reset()` 的顺序恢复连接默认状态。
    ///
    /// # 参数
    /// - `connection`：待复位的物理连接。
    /// - `keep_underlying_transaction_isolation`：是否保留借用者设置的隔离级别。
    ///
    /// # 错误
    /// 任一驱动复位或清理警告操作失败时立即返回错误；调用方必须丢弃连接。
    pub async fn reset(
        &self,
        connection: &mut dyn PhysicalConnection,
        keep_underlying_transaction_isolation: bool,
    ) -> Result<(), DruidError> {
        self.reset_state(connection, keep_underlying_transaction_isolation)
            .await?;

        if connection.capabilities().clear_warnings {
            connection.clear_warnings().await?;
        }

        Ok(())
    }

    /// 恢复连接属性，但暂不清理 warning。
    ///
    /// 对应 Java `DruidConnectionHolder#reset()` 中位于 statement trace 清理
    /// 之前的状态恢复阶段。holder 需要在该阶段之后关闭所有遗留 Statement，
    /// 再执行 `Connection#clearWarnings()`，因此不能把两个阶段合并。
    pub(crate) async fn reset_state(
        &self,
        connection: &mut dyn PhysicalConnection,
        keep_underlying_transaction_isolation: bool,
    ) -> Result<(), DruidError> {
        let capabilities = connection.capabilities();

        // 顺序严格对应 Java DruidConnectionHolder#reset。
        if capabilities.read_only && connection.read_only() != self.read_only {
            connection.set_read_only(self.read_only).await?;
        }

        if capabilities.holdability && connection.holdability() != self.holdability {
            connection.set_holdability(self.holdability).await?;
        }

        if !keep_underlying_transaction_isolation
            && capabilities.transaction_isolation
            && connection.transaction_isolation() != self.transaction_isolation
        {
            connection
                .set_transaction_isolation(self.transaction_isolation)
                .await?;
        }

        if capabilities.auto_commit && connection.auto_commit() != self.auto_commit {
            connection.set_auto_commit(self.auto_commit).await?;
        }

        Ok(())
    }
}

fn optional_standard_state<T>(result: Result<T, RdbcError>) -> Result<Option<T>, DruidError> {
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
