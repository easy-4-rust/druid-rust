//! Non-blocking local metrics runtime for Druid datasource observability.
//!
//! `druid-metrics` provides a background metrics pipeline that samples
//! [`DataSourceMonitorable`] instances on a configurable interval, coalesces
//! snapshots through a bounded queue, and exports timeline + Prometheus data.
//!
//! # Architecture
//!
//! ```text
//! [DataSource] --try_snapshot--> [Sampler] --bounded mpsc--> [Aggregator] --> [Exporter]
//! ```
//!
//! SQL hot-path methods never call `send().await`, network, or disk.
//! All metric collection is driven by the background sampler.

pub mod aggregator;
pub mod config;
pub mod error;
pub mod prometheus;
pub mod registry;
pub mod runtime;
pub mod sampler;
pub mod sanitizer;
pub mod self_metrics;
pub mod timeline;

pub use config::{DruidMetricsConfig, DruidMetricsConfigBuilder, SqlTextPolicy};
pub use error::{MetricsConfigError, MetricsError};
pub use registry::RegistrationGuard;
pub use runtime::DruidMetricsRuntime;
