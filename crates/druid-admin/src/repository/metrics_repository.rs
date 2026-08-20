//! In-memory metrics repository for the standalone admin.
//!
//! Stores the latest snapshot per datasource instance, keyed by
//! `(service_id, datasource_identity)`. Long-term history is delegated
//! to Prometheus; this repository only holds the most recent state.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::model::dto::{DataSourceContent, SqlListContent, WallResult};

/// Unique identifier for a datasource instance within the admin repository.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DataSourceId {
    /// Service instance identifier (e.g. pod name or host:port).
    pub service_id: String,
    /// Datasource identity within the service (Java `DataSource.getIdentity()`).
    pub identity: i64,
}

/// A single datasource entry stored in the repository.
#[derive(Clone, Debug)]
pub struct DataSourceEntry {
    /// Datasource pool statistics.
    pub datasource: DataSourceContent,
    /// SQL statistics for this datasource, keyed by SQL hash.
    pub sql_stats: HashMap<i64, SqlListContent>,
    /// Wall (firewall) statistics for this datasource.
    pub wall: WallResult,
    /// Timestamp of the last update (epoch millis).
    pub last_updated_ms: i64,
    /// Sequence number for deduplication.
    pub sequence: u64,
}

/// Thread-safe in-memory repository of latest datasource metrics.
///
/// All mutations acquire a write lock; all reads acquire a read lock.
/// For V1 single-instance admin this is sufficient.
#[derive(Clone)]
pub struct MetricsRepository {
    inner: Arc<RwLock<MetricsRepositoryInner>>,
}

struct MetricsRepositoryInner {
    entries: HashMap<DataSourceId, DataSourceEntry>,
    /// Global ingest counter for observability.
    ingest_count: u64,
    /// Count of rejected (stale/duplicate) batches.
    rejected_count: u64,
}

impl MetricsRepository {
    /// Create an empty repository.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsRepositoryInner {
                entries: HashMap::new(),
                ingest_count: 0,
                rejected_count: 0,
            })),
        }
    }

    /// Upsert a datasource entry. If the incoming sequence is not newer
    /// than the stored one, the update is rejected (deduplication).
    ///
    /// Returns `true` if the entry was accepted, `false` if rejected as stale.
    pub fn upsert_datasource(&self, id: DataSourceId, entry: DataSourceEntry) -> bool {
        let mut inner = self.inner.write();
        inner.ingest_count += 1;

        if let Some(existing) = inner.entries.get(&id) {
            if entry.sequence <= existing.sequence {
                inner.rejected_count += 1;
                return false;
            }
        }

        inner.entries.insert(id, entry);
        true
    }

    /// Get a snapshot of all datasource entries.
    pub fn all_datasources(&self) -> Vec<DataSourceEntry> {
        let inner = self.inner.read();
        inner.entries.values().cloned().collect()
    }

    /// Get a single datasource entry by id.
    pub fn get_datasource(&self, id: &DataSourceId) -> Option<DataSourceEntry> {
        let inner = self.inner.read();
        inner.entries.get(id).cloned()
    }

    /// Get all SQL stats across all datasources.
    pub fn all_sql_stats(&self) -> Vec<SqlListContent> {
        let inner = self.inner.read();
        inner
            .entries
            .values()
            .flat_map(|e| e.sql_stats.values().cloned())
            .collect()
    }

    /// Get the aggregated wall stats across all datasources.
    pub fn aggregated_wall(&self) -> WallResult {
        let inner = self.inner.read();
        let mut result = WallResult::default();
        for entry in inner.entries.values() {
            result.sum(&entry.wall);
        }
        result
    }

    /// Get ingest counters.
    pub fn counters(&self) -> (u64, u64) {
        let inner = self.inner.read();
        (inner.ingest_count, inner.rejected_count)
    }

    /// Get the number of stored datasource entries.
    pub fn len(&self) -> usize {
        let inner = self.inner.read();
        inner.entries.len()
    }

    /// Check if the repository is empty.
    pub fn is_empty(&self) -> bool {
        let inner = self.inner.read();
        inner.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&self) {
        let mut inner = self.inner.write();
        inner.entries.clear();
    }
}

impl Default for MetricsRepository {
    fn default() -> Self {
        Self::new()
    }
}
