//! 对应 Java：`com.alibaba.druid.proxy.rdbc.DatabaseMetaDataProxyImpl`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/proxy/rdbc/DatabaseMetaDataProxyImpl.java`。

use super::{DruidError, PhysicalDatabaseMetaData};
use std::ops::{Deref, DerefMut};

/// 借用当前物理连接的数据库元数据代理。
///
/// Java 对象是 `java.sql.DatabaseMetaData` 的纯委托包装。Rust 通过
/// `PhysicalDatabaseMetaData` 隔离 SQLx、RBDC、Toasty 等驱动差异，并用
/// 生命周期保证 metadata 不会超过其物理连接租约。除源实现的三个历史委托
/// 分支外，其余 173 个方法通过 `Deref/DerefMut` 直接到达 raw SPI。
pub struct DatabaseMetaDataProxyImpl<'connection> {
    raw: Box<dyn PhysicalDatabaseMetaData + 'connection>,
    connection_id: u64,
}

impl<'connection> DatabaseMetaDataProxyImpl<'connection> {
    /// 包装物理 metadata，并绑定产生它的池化连接身份。
    ///
    /// # 参数
    /// - `raw`：驱动 Adapter 创建的真实 metadata SPI。
    /// - `connection_id`：当前 `DruidPooledConnection` 的稳定 ID。
    pub fn new(raw: Box<dyn PhysicalDatabaseMetaData + 'connection>, connection_id: u64) -> Self {
        Self { raw, connection_id }
    }

    /// 返回产生 metadata 的池化连接 ID。
    ///
    /// 对应 Java：`DatabaseMetaData#getConnection()` 的连接身份语义。Rust
    /// 不复制或重新池化连接；调用方仍持有原 `DruidPooledConnection`。
    #[must_use]
    pub const fn get_connection_id(&self) -> u64 {
        self.connection_id
    }

    /// 返回 raw 物理 metadata SPI。
    #[must_use]
    pub fn raw(&self) -> &dyn PhysicalDatabaseMetaData {
        self.raw.as_ref()
    }

    /// 返回 raw 物理 metadata SPI 的可变引用。
    pub fn raw_mut(&mut self) -> &mut dyn PhysicalDatabaseMetaData {
        self.raw.as_mut()
    }

    /// 保留 Java 源实现的历史委托：
    /// `storesLowerCaseIdentifiers()` 实际调用
    /// `storesMixedCaseIdentifiers()`。
    pub async fn stores_lower_case_identifiers(&mut self) -> Result<bool, DruidError> {
        self.raw.stores_mixed_case_identifiers().await
    }

    /// 保留 Java 源实现的历史委托：
    /// `storesMixedCaseIdentifiers()` 实际调用
    /// `supportsMixedCaseIdentifiers()`。
    pub async fn stores_mixed_case_identifiers(&mut self) -> Result<bool, DruidError> {
        self.raw.supports_mixed_case_identifiers().await
    }

    /// 保留 Java 源实现的历史委托：
    /// `storesUpperCaseQuotedIdentifiers()` 实际调用
    /// `storesUpperCaseIdentifiers()`。
    pub async fn stores_upper_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        self.raw.stores_upper_case_identifiers().await
    }
}

impl std::fmt::Debug for DatabaseMetaDataProxyImpl<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatabaseMetaDataProxyImpl")
            .field("connection_id", &self.connection_id)
            .finish_non_exhaustive()
    }
}

impl<'connection> Deref for DatabaseMetaDataProxyImpl<'connection> {
    type Target = dyn PhysicalDatabaseMetaData + 'connection;

    fn deref(&self) -> &Self::Target {
        self.raw.as_ref()
    }
}

impl DerefMut for DatabaseMetaDataProxyImpl<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.raw.as_mut()
    }
}
