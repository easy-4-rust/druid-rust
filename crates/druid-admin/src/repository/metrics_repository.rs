//! Standalone Admin 的有界最新指标 repository。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use parking_lot::RwLock;

use crate::model::dto::{DataSourceContent, SqlListContent, WallResult};

use super::{MetricsRepositoryConfig, StoredDataSourceEntry};

/// Admin repository 内 datasource 的复合标识。
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct DataSourceId {
    /// 协议 stream service identity。
    pub service_id: String,
    /// service 内 datasource identity。
    pub identity: i64,
}

/// Repository 保存的一个 datasource 最新投影。
#[derive(Clone, Debug)]
pub struct DataSourceEntry {
    /// Datasource pool statistics。
    pub datasource: DataSourceContent,
    /// SQL statistics，key 为 SQL hash。
    pub sql_stats: HashMap<i64, SqlListContent>,
    /// Wall statistics。
    pub wall: WallResult,
    /// Client sampling timestamp；TTL 使用 server receive monotonic time。
    pub last_updated_ms: i64,
    /// Protocol sequence。
    pub sequence: u64,
}

/// Thread-safe、容量与 TTL 显式受限的最新指标 repository。
#[derive(Clone)]
pub struct MetricsRepository {
    inner: Arc<RwLock<MetricsRepositoryInner>>,
    config: MetricsRepositoryConfig,
}

struct MetricsRepositoryInner {
    entries: HashMap<DataSourceId, StoredDataSourceEntry>,
    ingest_count: u64,
    rejected_count: u64,
    capacity_rejected_count: u64,
    ttl_evicted_count: u64,
    metrics_output_rejected_count: u64,
}

impl MetricsRepository {
    /// 使用生产默认预算创建空 repository。
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(MetricsRepositoryConfig::default())
    }

    /// 使用已验证预算创建空 repository。
    #[must_use]
    pub fn with_config(config: MetricsRepositoryConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(MetricsRepositoryInner {
                entries: HashMap::new(),
                ingest_count: 0,
                rejected_count: 0,
                capacity_rejected_count: 0,
                ttl_evicted_count: 0,
                metrics_output_rejected_count: 0,
            })),
            config,
        }
    }

    /// Upsert 一个 datasource；stale、duplicate 或预算超限时原子拒绝。
    pub fn upsert_datasource(&self, id: DataSourceId, entry: DataSourceEntry) -> bool {
        self.apply_batch(vec![(id, entry)])
    }

    /// 在一个写锁事务中校验并应用完整 incremental batch。
    pub fn apply_batch(&self, entries: Vec<(DataSourceId, DataSourceEntry)>) -> bool {
        let mut inner = self.inner.write();
        self.prune_expired(&mut inner);
        inner.ingest_count = inner.ingest_count.saturating_add(1);
        let mut seen = HashSet::with_capacity(entries.len());
        let invalid = entries.iter().any(|(id, entry)| {
            !seen.insert(id.clone())
                || inner
                    .entries
                    .get(id)
                    .is_some_and(|existing| entry.sequence <= existing.entry.sequence)
        });
        if invalid {
            inner.rejected_count = inner.rejected_count.saturating_add(1);
            return false;
        }
        if self.exceeds_limits(&inner, &entries, None) {
            inner.rejected_count = inner.rejected_count.saturating_add(1);
            inner.capacity_rejected_count = inner.capacity_rejected_count.saturating_add(1);
            return false;
        }
        for (id, entry) in entries {
            inner.entries.insert(id, StoredDataSourceEntry::new(entry));
        }
        true
    }

    /// 原子替换一个协议 stream 的全部 datasource；空集合表示权威清空。
    pub fn replace_stream(
        &self,
        service_id: &str,
        entries: Vec<(DataSourceId, DataSourceEntry)>,
    ) -> bool {
        let mut inner = self.inner.write();
        self.prune_expired(&mut inner);
        inner.ingest_count = inner.ingest_count.saturating_add(1);
        let mut seen = HashSet::with_capacity(entries.len());
        let invalid = entries
            .iter()
            .any(|(id, _)| id.service_id != service_id || !seen.insert(id.clone()));
        if invalid {
            inner.rejected_count = inner.rejected_count.saturating_add(1);
            return false;
        }
        if self.exceeds_limits(&inner, &entries, Some(service_id)) {
            inner.rejected_count = inner.rejected_count.saturating_add(1);
            inner.capacity_rejected_count = inner.capacity_rejected_count.saturating_add(1);
            return false;
        }
        inner.entries.retain(|id, _| id.service_id != service_id);
        for (id, entry) in entries {
            inner.entries.insert(id, StoredDataSourceEntry::new(entry));
        }
        true
    }

    /// 返回所有未过期 datasource 最新值。
    pub fn all_datasources(&self) -> Vec<DataSourceEntry> {
        let mut inner = self.inner.write();
        self.prune_expired(&mut inner);
        inner
            .entries
            .values()
            .map(|stored| stored.entry.clone())
            .collect()
    }

    /// 通过 service + datasource identity 精确查询。
    pub fn get_datasource(&self, id: &DataSourceId) -> Option<DataSourceEntry> {
        let mut inner = self.inner.write();
        self.prune_expired(&mut inner);
        inner.entries.get(id).map(|stored| stored.entry.clone())
    }

    /// 通过 service + datasource + SQL id/hash 精确查询 SQL detail。
    pub fn get_sql_stat(
        &self,
        service_id: &str,
        datasource_identity: i64,
        sql_identity: i64,
    ) -> Option<SqlListContent> {
        self.get_datasource(&DataSourceId {
            service_id: service_id.to_owned(),
            identity: datasource_identity,
        })
        .and_then(|entry| {
            entry
                .sql_stats
                .values()
                .find(|stat| stat.id == sql_identity || stat.hash == sql_identity)
                .cloned()
        })
    }

    /// 返回所有未过期 SQL 最新值。
    pub fn all_sql_stats(&self) -> Vec<SqlListContent> {
        self.all_datasources()
            .into_iter()
            .flat_map(|entry| entry.sql_stats.into_values())
            .collect()
    }

    /// 汇总所有未过期 Wall statistics。
    pub fn aggregated_wall(&self) -> WallResult {
        let mut result = WallResult::default();
        for entry in self.all_datasources() {
            result.sum(&entry.wall);
        }
        result
    }

    /// 返回 ingest/rejected batch counters。
    pub fn counters(&self) -> (u64, u64) {
        let inner = self.inner.read();
        (inner.ingest_count, inner.rejected_count)
    }

    /// 返回 capacity rejected、TTL evicted、metrics output rejected counters。
    pub fn limit_counters(&self) -> (u64, u64, u64) {
        let inner = self.inner.read();
        (
            inner.capacity_rejected_count,
            inner.ttl_evicted_count,
            inner.metrics_output_rejected_count,
        )
    }

    /// 校验一次完整 `/metrics` 渲染大小；超限时计数并返回 false。
    pub fn accept_metrics_output_size(&self, bytes: usize) -> bool {
        if bytes <= self.config.max_metrics_output_bytes() {
            return true;
        }
        let mut inner = self.inner.write();
        inner.metrics_output_rejected_count = inner.metrics_output_rejected_count.saturating_add(1);
        false
    }

    /// 返回 `/metrics` 字节预算。
    #[must_use]
    pub const fn max_metrics_output_bytes(&self) -> usize {
        self.config.max_metrics_output_bytes()
    }

    /// 返回允许 IngestHandler 保存的最大 service stream 数。
    #[must_use]
    pub const fn max_services(&self) -> usize {
        self.config.max_services()
    }

    /// 返回一个 authoritative transaction 的最大 datasource 数。
    #[must_use]
    pub const fn max_datasources_per_service(&self) -> usize {
        self.config.max_datasources_per_service()
    }

    /// 返回一个 datasource 的最大 SQL identity 数。
    #[must_use]
    pub const fn max_sql_entries_per_datasource(&self) -> usize {
        self.config.max_sql_entries_per_datasource()
    }

    /// 返回未过期 datasource 数。
    pub fn len(&self) -> usize {
        self.all_datasources().len()
    }

    /// 返回 repository 是否没有未过期 datasource。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 清空所有最新值，保留 observable counters。
    pub fn clear(&self) {
        self.inner.write().entries.clear();
    }

    fn prune_expired(&self, inner: &mut MetricsRepositoryInner) {
        let now = Instant::now();
        let before = inner.entries.len();
        inner
            .entries
            .retain(|_, stored| now.duration_since(stored.received_at) < self.config.entry_ttl());
        inner.ttl_evicted_count = inner
            .ttl_evicted_count
            .saturating_add((before.saturating_sub(inner.entries.len())) as u64);
    }

    fn exceeds_limits(
        &self,
        inner: &MetricsRepositoryInner,
        incoming: &[(DataSourceId, DataSourceEntry)],
        replaced_service: Option<&str>,
    ) -> bool {
        if incoming
            .iter()
            .any(|(_, entry)| entry.sql_stats.len() > self.config.max_sql_entries_per_datasource())
        {
            return true;
        }
        let mut identities = inner
            .entries
            .keys()
            .filter(|id| replaced_service != Some(id.service_id.as_str()))
            .cloned()
            .collect::<HashSet<_>>();
        identities.extend(incoming.iter().map(|(id, _)| id.clone()));
        let services = identities
            .iter()
            .map(|id| id.service_id.as_str())
            .collect::<HashSet<_>>();
        if services.len() > self.config.max_services() {
            return true;
        }
        services.into_iter().any(|service_id| {
            identities
                .iter()
                .filter(|id| id.service_id.as_str() == service_id)
                .count()
                > self.config.max_datasources_per_service()
        })
    }
}

impl Default for MetricsRepository {
    fn default() -> Self {
        Self::new()
    }
}
