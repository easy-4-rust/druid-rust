//! 对应 Java：`com.alibaba.druid.proxy.rdbc.DatabaseMetaDataProxyImpl`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/proxy/rdbc/DatabaseMetaDataProxyImpl.java`。

use super::standard_resource_registry::StandardResourceRegistry;
use super::{DruidPooledResultSet, FilterChain, PhysicalDatabaseMetaData, RdbcError};
use crate::stats::StatsCollector;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 借用当前物理连接的数据库元数据代理。
///
/// Java 对象是 `java.sql.DatabaseMetaData` 的纯委托包装。Rust 通过
/// `PhysicalDatabaseMetaData` 隔离 SQLx、RBDC、Toasty 等驱动差异，并用
/// 生命周期保证 metadata 不会超过其物理连接租约。除源实现的三个历史委托
/// 分支外，其余 173 个方法通过 `Deref/DerefMut` 直接到达 raw SPI。
pub struct DatabaseMetaDataProxyImpl<'connection> {
    raw: Option<Box<dyn PhysicalDatabaseMetaData + 'connection>>,
    standard: Option<Box<dyn druid_rdbc::metadata::DatabaseMetaData>>,
    lease_active: Arc<AtomicBool>,
    connection_id: u64,
    result_set_id_seed: Option<Arc<std::sync::atomic::AtomicU64>>,
    standard_resources: Option<Arc<StandardResourceRegistry>>,
    standard_policy: Option<Arc<super::standard_logical_policy::StandardLogicalPolicy>>,
    filter_chain: Option<Arc<FilterChain>>,
    stats_collector: Option<Arc<StatsCollector>>,
}

impl<'connection> DatabaseMetaDataProxyImpl<'connection> {
    /// 包装物理 metadata，并绑定产生它的池化连接身份。
    ///
    /// # 参数
    /// - `raw`：驱动 Adapter 创建的真实 metadata SPI。
    /// - `connection_id`：当前 `DruidPooledConnection` 的稳定 ID。
    pub fn new(raw: Box<dyn PhysicalDatabaseMetaData + 'connection>, connection_id: u64) -> Self {
        Self {
            raw: Some(raw),
            standard: None,
            lease_active: Arc::new(AtomicBool::new(true)),
            connection_id,
            result_set_id_seed: None,
            standard_resources: None,
            standard_policy: None,
            filter_chain: None,
            stats_collector: None,
        }
    }

    /// 创建拥有 logical lease/actor 的 canonical live metadata proxy。
    pub(crate) fn new_standard(
        standard: Box<dyn druid_rdbc::metadata::DatabaseMetaData>,
        connection_id: u64,
        lease_active: Arc<AtomicBool>,
        result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
        standard_resources: Arc<StandardResourceRegistry>,
        standard_policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<StatsCollector>>,
    ) -> DatabaseMetaDataProxyImpl<'static> {
        DatabaseMetaDataProxyImpl {
            raw: None,
            standard: Some(standard),
            lease_active,
            connection_id,
            result_set_id_seed: Some(result_set_id_seed),
            standard_resources: Some(standard_resources),
            standard_policy: Some(standard_policy),
            filter_chain,
            stats_collector,
        }
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
    pub fn raw(&self) -> &(dyn PhysicalDatabaseMetaData + 'connection) {
        self.raw
            .as_deref()
            .expect("physical metadata constructor owns raw metadata")
    }

    /// 返回 raw 物理 metadata SPI 的可变引用。
    pub fn raw_mut(&mut self) -> &mut (dyn PhysicalDatabaseMetaData + 'connection) {
        self.raw
            .as_deref_mut()
            .expect("physical metadata constructor owns raw metadata")
    }

    /// 保留 Java 源实现的历史委托：
    /// `storesLowerCaseIdentifiers()` 实际调用
    /// `storesMixedCaseIdentifiers()`。
    pub async fn stores_lower_case_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.raw_mut().stores_mixed_case_identifiers().await
    }

    /// 保留 Java 源实现的历史委托：
    /// `storesMixedCaseIdentifiers()` 实际调用
    /// `supportsMixedCaseIdentifiers()`。
    pub async fn stores_mixed_case_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.raw_mut().supports_mixed_case_identifiers().await
    }

    /// 保留 Java 源实现的历史委托：
    /// `storesUpperCaseQuotedIdentifiers()` 实际调用
    /// `storesUpperCaseIdentifiers()`。
    pub async fn stores_upper_case_quoted_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.raw_mut().stores_upper_case_identifiers().await
    }
}

impl std::fmt::Debug for DatabaseMetaDataProxyImpl<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatabaseMetaDataProxyImpl")
            .field("connection_id", &self.connection_id)
            .field("standard", &self.standard.is_some())
            .finish_non_exhaustive()
    }
}

impl<'connection> Deref for DatabaseMetaDataProxyImpl<'connection> {
    type Target = dyn PhysicalDatabaseMetaData + 'connection;

    fn deref(&self) -> &Self::Target {
        self.raw()
    }
}

impl DerefMut for DatabaseMetaDataProxyImpl<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.raw_mut()
    }
}

impl druid_rdbc::wrapper::Wrapper for DatabaseMetaDataProxyImpl<'static> {
    fn wrapper_view(&self) -> &dyn druid_rdbc::wrapper::Wrapper {
        self
    }

    fn canonical_unwrap(
        &self,
        target: std::any::TypeId,
    ) -> Option<druid_rdbc::wrapper::StandardUnwrapped<'_>> {
        (target == std::any::TypeId::of::<dyn druid_rdbc::metadata::DatabaseMetaData>()).then_some(
            druid_rdbc::wrapper::StandardUnwrapped::DatabaseMetaData(self),
        )
    }
}

#[async_trait::async_trait]
impl druid_rdbc::metadata::DatabaseMetaData for DatabaseMetaDataProxyImpl<'static> {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        if !self.lease_active.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.standard
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .database_product_name()
            .await
    }

    async fn tables(
        &mut self,
        query: druid_rdbc::metadata::TablesQuery,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        if !self.lease_active.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let resources = self
            .standard_resources
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let admission = resources.begin_admission(policy.resource_generation())?;
        let raw = self
            .standard
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .tables(query)
            .await?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard_metadata(
                self.connection_id,
                self.result_set_id_seed
                    .as_ref()
                    .ok_or(RdbcError::ConnectionDiscarded)?
                    .clone(),
                Arc::clone(&self.lease_active),
                self.filter_chain.clone(),
                self.stats_collector.clone(),
                resources,
                policy,
                &admission,
                raw,
            )
            .await?,
        ))
    }
}
