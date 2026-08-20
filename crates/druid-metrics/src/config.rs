use std::time::Duration;

use crate::error::MetricsConfigError;

/// Policy for including SQL text in exported metrics.
///
/// SQL text is always kept in the internal repository payload for diagnostics,
/// but this policy controls what appears in Prometheus labels and external APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SqlTextPolicy {
    /// Do not include any SQL text in exports.
    Disabled,
    /// Include only the parameterized/fingerprint form (default).
    #[default]
    FingerprintOnly,
    /// Include raw SQL without bind parameter values.
    RawWithoutParameters,
}

/// Builder for [`DruidMetricsConfig`].
///
/// Created via [`DruidMetricsConfig::builder()`].
#[derive(Debug, Clone)]
pub struct DruidMetricsConfigBuilder {
    sample_interval: Duration,
    queue_capacity: usize,
    batch_size: usize,
    coalesce_window: Duration,
    max_sql_statements: usize,
    sql_text_policy: SqlTextPolicy,
    shutdown_timeout: Duration,
}

impl DruidMetricsConfigBuilder {
    fn new() -> Self {
        let defaults = DruidMetricsConfig::defaults();
        Self {
            sample_interval: defaults.sample_interval,
            queue_capacity: defaults.queue_capacity,
            batch_size: defaults.batch_size,
            coalesce_window: defaults.coalesce_window,
            max_sql_statements: defaults.max_sql_statements,
            sql_text_policy: defaults.sql_text_policy,
            shutdown_timeout: defaults.shutdown_timeout,
        }
    }

    /// Set the sampling interval. Must be non-zero.
    pub fn sample_interval(mut self, d: Duration) -> Self {
        self.sample_interval = d;
        self
    }

    /// Set the bounded queue capacity. Must be > 0.
    pub fn queue_capacity(mut self, n: usize) -> Self {
        self.queue_capacity = n;
        self
    }

    /// Set the batch size for exporter. Must be > 0.
    pub fn batch_size(mut self, n: usize) -> Self {
        self.batch_size = n;
        self
    }

    /// Set the coalescing window. Must be non-zero.
    pub fn coalesce_window(mut self, d: Duration) -> Self {
        self.coalesce_window = d;
        self
    }

    /// Set the maximum number of tracked SQL statements per datasource.
    pub fn max_sql_statements(mut self, n: usize) -> Self {
        self.max_sql_statements = n;
        self
    }

    /// Set the SQL text export policy.
    pub fn sql_text_policy(mut self, p: SqlTextPolicy) -> Self {
        self.sql_text_policy = p;
        self
    }

    /// Set the graceful shutdown timeout. Must be non-zero.
    pub fn shutdown_timeout(mut self, d: Duration) -> Self {
        self.shutdown_timeout = d;
        self
    }

    /// Validate and build the configuration.
    pub fn build(self) -> Result<DruidMetricsConfig, MetricsConfigError> {
        if self.sample_interval.is_zero() {
            return Err(MetricsConfigError::InvalidSampleInterval);
        }
        if self.queue_capacity == 0 {
            return Err(MetricsConfigError::InvalidQueueCapacity);
        }
        if self.batch_size == 0 {
            return Err(MetricsConfigError::InvalidBatchSize);
        }
        if self.coalesce_window.is_zero() {
            return Err(MetricsConfigError::InvalidCoalesceWindow);
        }
        if self.shutdown_timeout.is_zero() {
            return Err(MetricsConfigError::InvalidShutdownTimeout);
        }

        Ok(DruidMetricsConfig {
            sample_interval: self.sample_interval,
            queue_capacity: self.queue_capacity,
            batch_size: self.batch_size,
            coalesce_window: self.coalesce_window,
            max_sql_statements: self.max_sql_statements,
            sql_text_policy: self.sql_text_policy,
            shutdown_timeout: self.shutdown_timeout,
        })
    }
}

/// Configuration for the Druid metrics runtime.
///
/// Created via the builder pattern:
///
/// ```rust
/// use druid_metrics::{DruidMetricsConfig, SqlTextPolicy};
/// use std::time::Duration;
///
/// let cfg = DruidMetricsConfig::builder()
///     .sample_interval(Duration::from_secs(30))
///     .queue_capacity(2048)
///     .build()
///     .expect("valid config");
///
/// assert_eq!(cfg.sample_interval, Duration::from_secs(30));
/// ```
#[derive(Debug, Clone)]
pub struct DruidMetricsConfig {
    /// How often the sampler polls registered datasources.
    pub sample_interval: Duration,
    /// Bounded channel capacity between sampler and aggregator.
    pub queue_capacity: usize,
    /// Number of snapshots per exporter batch.
    pub batch_size: usize,
    /// Coalescing window for deduplicating per-datasource snapshots.
    pub coalesce_window: Duration,
    /// Maximum number of SQL statements tracked per datasource.
    pub max_sql_statements: usize,
    /// Policy for including SQL text in exported metrics.
    pub sql_text_policy: SqlTextPolicy,
    /// Maximum time to wait for graceful shutdown.
    pub shutdown_timeout: Duration,
}

impl DruidMetricsConfig {
    /// Create a builder with default values.
    pub fn builder() -> DruidMetricsConfigBuilder {
        DruidMetricsConfigBuilder::new()
    }

    /// Returns the default configuration values.
    fn defaults() -> Self {
        Self {
            sample_interval: Duration::from_secs(15),
            queue_capacity: 1024,
            batch_size: 64,
            coalesce_window: Duration::from_millis(500),
            max_sql_statements: 256,
            sql_text_policy: SqlTextPolicy::FingerprintOnly,
            shutdown_timeout: Duration::from_secs(5),
        }
    }
}

impl Default for DruidMetricsConfig {
    fn default() -> Self {
        Self::defaults()
    }
}
