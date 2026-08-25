//! 物理连接默认状态快照。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidConnectionHolder` 中的
//! `defaultReadOnly`、`defaultHoldability`、`defaultTransactionIsolation`
//! 与 `defaultAutoCommit` 字段，以及 `DruidConnectionHolder#reset()`。

use super::{DruidError, PhysicalConnection};
use druid_rdbc::connection::Connection as StandardConnection;

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
        }
    }

    /// 从 canonical standard connection 捕获 Druid 回收所需默认状态。
    pub(crate) fn capture_standard(
        connection: &dyn StandardConnection,
    ) -> Result<Self, DruidError> {
        Ok(Self {
            auto_commit: connection.auto_commit()?,
            read_only: connection.read_only()?,
            holdability: 0,
            transaction_isolation: 2,
            catalog: connection.catalog()?,
            schema: connection.schema()?,
        })
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
        let capabilities = connection.capabilities();
        Ok(connection.read_only()? != self.read_only
            || connection.auto_commit()? != self.auto_commit
            || (capabilities.catalogs && connection.catalog()?.as_deref() != self.catalog())
            || (capabilities.schemas && connection.schema()?.as_deref() != self.schema()))
    }

    /// 按 Druid holder 顺序恢复 canonical standard connection 状态。
    pub(crate) async fn reset_standard_state(
        &self,
        connection: &mut dyn StandardConnection,
    ) -> Result<(), DruidError> {
        let capabilities = connection.capabilities();
        if connection.read_only()? != self.read_only {
            connection.set_read_only(self.read_only).await?;
        }
        if capabilities.catalogs && connection.catalog()?.as_deref() != self.catalog() {
            connection.set_catalog(self.catalog()).await?;
        }
        if capabilities.schemas && connection.schema()?.as_deref() != self.schema() {
            connection.set_schema(self.schema()).await?;
        }
        if connection.auto_commit()? != self.auto_commit {
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
