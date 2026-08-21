use std::time::Duration;

/// 默认最多保留的 service identity 数。
pub const DEFAULT_MAX_SERVICES: usize = 1_024;
/// 每个 service 默认最多保留的 datasource 数。
pub const DEFAULT_MAX_DATASOURCES_PER_SERVICE: usize = 256;
/// 每个 datasource 默认最多保留的 SQL identity 数。
pub const DEFAULT_MAX_SQL_ENTRIES_PER_DATASOURCE: usize = 1_024;
/// repository 最新状态默认 TTL（15 分钟）。
pub const DEFAULT_ENTRY_TTL: Duration = Duration::from_secs(15 * 60);
/// `/metrics` 默认最大渲染字节数（1 MiB）。
pub const DEFAULT_MAX_METRICS_OUTPUT_BYTES: usize = 1 << 20;

/// Admin 最新指标 repository 的容量、TTL 与输出预算。
///
/// Rust-only 运行配置对象；所有上限必须非零，避免“0 表示无限”的旁路。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricsRepositoryConfig {
    max_services: usize,
    max_datasources_per_service: usize,
    max_sql_entries_per_datasource: usize,
    entry_ttl: Duration,
    max_metrics_output_bytes: usize,
}

impl MetricsRepositoryConfig {
    /// 创建并验证 repository 预算。
    pub fn try_new(
        max_services: usize,
        max_datasources_per_service: usize,
        max_sql_entries_per_datasource: usize,
        entry_ttl: Duration,
        max_metrics_output_bytes: usize,
    ) -> Result<Self, &'static str> {
        if max_services == 0
            || max_datasources_per_service == 0
            || max_sql_entries_per_datasource == 0
            || entry_ttl.is_zero()
            || max_metrics_output_bytes == 0
        {
            return Err("repository capacities, TTL, and output budget must be non-zero");
        }
        Ok(Self {
            max_services,
            max_datasources_per_service,
            max_sql_entries_per_datasource,
            entry_ttl,
            max_metrics_output_bytes,
        })
    }

    /// 返回最大 service identity 数。
    #[must_use]
    pub const fn max_services(&self) -> usize {
        self.max_services
    }

    /// 返回每个 service 的最大 datasource 数。
    #[must_use]
    pub const fn max_datasources_per_service(&self) -> usize {
        self.max_datasources_per_service
    }

    /// 返回每个 datasource 的最大 SQL 数。
    #[must_use]
    pub const fn max_sql_entries_per_datasource(&self) -> usize {
        self.max_sql_entries_per_datasource
    }

    /// 返回最新状态 TTL。
    #[must_use]
    pub const fn entry_ttl(&self) -> Duration {
        self.entry_ttl
    }

    /// 返回 `/metrics` 最大渲染字节数。
    #[must_use]
    pub const fn max_metrics_output_bytes(&self) -> usize {
        self.max_metrics_output_bytes
    }
}

impl Default for MetricsRepositoryConfig {
    fn default() -> Self {
        Self {
            max_services: DEFAULT_MAX_SERVICES,
            max_datasources_per_service: DEFAULT_MAX_DATASOURCES_PER_SERVICE,
            max_sql_entries_per_datasource: DEFAULT_MAX_SQL_ENTRIES_PER_DATASOURCE,
            entry_ttl: DEFAULT_ENTRY_TTL,
            max_metrics_output_bytes: DEFAULT_MAX_METRICS_OUTPUT_BYTES,
        }
    }
}
