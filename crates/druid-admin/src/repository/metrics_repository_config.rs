use std::time::Duration;

/// 默认最多保留的 service identity 数。
pub const DEFAULT_MAX_SERVICES: usize = 1_024;
/// 每个 service 默认最多保留的 datasource 数。
pub const DEFAULT_MAX_DATASOURCES_PER_SERVICE: usize = 256;
/// 每个 datasource 默认最多保留的 SQL identity 数。
pub const DEFAULT_MAX_SQL_ENTRIES_PER_DATASOURCE: usize = 1_024;
/// 仓库默认最多保留的 datasource 总数。
pub const DEFAULT_MAX_TOTAL_DATASOURCES: usize = 4_096;
/// 仓库默认最多保留的 SQL identity 总数。
pub const DEFAULT_MAX_TOTAL_SQL_ENTRIES: usize = 16_384;
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
    max_total_datasources: usize,
    max_total_sql_entries: usize,
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
            max_total_datasources: DEFAULT_MAX_TOTAL_DATASOURCES,
            max_total_sql_entries: DEFAULT_MAX_TOTAL_SQL_ENTRIES,
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

    /// 设置整个仓库的 datasource 与 SQL 总量上限。
    ///
    /// 两个参数分别是允许保留的 datasource 和 SQL identity 数；均须大于零。
    /// 返回验证后的新配置，非法上限返回错误。
    pub fn with_global_limits(
        mut self,
        max_total_datasources: usize,
        max_total_sql_entries: usize,
    ) -> Result<Self, &'static str> {
        if max_total_datasources == 0 || max_total_sql_entries == 0 {
            return Err("global datasource and SQL capacities must be non-zero");
        }
        self.max_total_datasources = max_total_datasources;
        self.max_total_sql_entries = max_total_sql_entries;
        Ok(self)
    }

    /// 返回整个仓库最多保留的 datasource 总数。
    #[must_use]
    pub const fn max_total_datasources(&self) -> usize {
        self.max_total_datasources
    }

    /// 返回整个仓库最多保留的 SQL identity 总数。
    #[must_use]
    pub const fn max_total_sql_entries(&self) -> usize {
        self.max_total_sql_entries
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
            max_total_datasources: DEFAULT_MAX_TOTAL_DATASOURCES,
            max_total_sql_entries: DEFAULT_MAX_TOTAL_SQL_ENTRIES,
            entry_ttl: DEFAULT_ENTRY_TTL,
            max_metrics_output_bytes: DEFAULT_MAX_METRICS_OUTPUT_BYTES,
        }
    }
}
