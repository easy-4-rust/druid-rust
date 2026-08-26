//! Canonical pooled DatabaseMetaData proxy.

use super::standard_resource_registry::StandardResourceRegistry;
use super::{DruidPooledResultSet, FilterChain, RdbcError};
use crate::stats::StatsCollector;
use druid_rdbc::metadata::{DatabaseMetaData, DatabaseMetaDataRequest, RowIdLifetime, TablesQuery};
use druid_rdbc::result_set::ResultSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Logical metadata handle owned by one Druid connection lease.
pub struct DatabaseMetaDataProxyImpl {
    raw: Box<dyn DatabaseMetaData>,
    lease_active: Arc<AtomicBool>,
    connection_id: u64,
    result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
    resources: Arc<StandardResourceRegistry>,
    policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
    filter_chain: Option<Arc<FilterChain>>,
    stats_collector: Option<Arc<StatsCollector>>,
}

impl DatabaseMetaDataProxyImpl {
    /// Creates a live canonical metadata proxy.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_standard(
        raw: Box<dyn DatabaseMetaData>,
        connection_id: u64,
        lease_active: Arc<AtomicBool>,
        result_set_id_seed: Arc<std::sync::atomic::AtomicU64>,
        resources: Arc<StandardResourceRegistry>,
        policy: Arc<super::standard_logical_policy::StandardLogicalPolicy>,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Option<Arc<StatsCollector>>,
    ) -> Self {
        Self {
            raw,
            lease_active,
            connection_id,
            result_set_id_seed,
            resources,
            policy,
            filter_chain,
            stats_collector,
        }
    }

    /// Returns the Druid connection identity that created this metadata handle.
    #[must_use]
    pub const fn get_connection_id(&self) -> u64 {
        self.connection_id
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.lease_active.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(RdbcError::ConnectionDiscarded)
        }
    }
}

impl std::fmt::Debug for DatabaseMetaDataProxyImpl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatabaseMetaDataProxyImpl")
            .field("connection_id", &self.connection_id)
            .field("lease_active", &self.lease_active.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

druid_rdbc::impl_standard_wrapper!(DatabaseMetaDataProxyImpl => DatabaseMetaData);

#[async_trait::async_trait]
impl DatabaseMetaData for DatabaseMetaDataProxyImpl {
    async fn metadata_boolean(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.policy.classify(
            self.raw.metadata_boolean(request).await,
            Some("DatabaseMetaData.boolean"),
        )
    }

    async fn metadata_string(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        self.policy.classify(
            self.raw.metadata_string(request).await,
            Some("DatabaseMetaData.string"),
        )
    }

    async fn metadata_integer(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        self.policy.classify(
            self.raw.metadata_integer(request).await,
            Some("DatabaseMetaData.integer"),
        )
    }

    async fn metadata_result_set(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let operation = request.operation().as_str();
        let admission = self
            .resources
            .begin_admission(self.policy.resource_generation())?;
        let raw = self
            .policy
            .classify(self.raw.metadata_result_set(request).await, Some(operation))?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard_metadata(
                self.connection_id,
                Arc::clone(&self.result_set_id_seed),
                Arc::clone(&self.lease_active),
                self.filter_chain.clone(),
                self.stats_collector.clone(),
                Arc::clone(&self.resources),
                Arc::clone(&self.policy),
                &admission,
                raw,
            )
            .await?,
        ))
    }

    async fn metadata_row_id_lifetime(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<RowIdLifetime, RdbcError> {
        self.ensure_open()?;
        self.policy.classify(
            self.raw.metadata_row_id_lifetime(request).await,
            Some("DatabaseMetaData.row_id_lifetime"),
        )
    }

    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        self.ensure_open()?;
        self.policy
            .classify(self.raw.database_product_name().await, None)
    }

    async fn tables(&mut self, query: TablesQuery) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        let admission = self
            .resources
            .begin_admission(self.policy.resource_generation())?;
        let raw = self.policy.classify(
            self.raw.tables(query).await,
            Some("DatabaseMetaData.tables"),
        )?;
        Ok(Box::new(
            DruidPooledResultSet::new_standard_metadata(
                self.connection_id,
                Arc::clone(&self.result_set_id_seed),
                Arc::clone(&self.lease_active),
                self.filter_chain.clone(),
                self.stats_collector.clone(),
                Arc::clone(&self.resources),
                Arc::clone(&self.policy),
                &admission,
                raw,
            )
            .await?,
        ))
    }
}
