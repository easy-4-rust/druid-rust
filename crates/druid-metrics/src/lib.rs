//! Non-blocking local metrics runtime for Druid datasource observability.
//!
//! `druid-metrics` provides a background metrics pipeline that samples
//! [`DataSourceMonitorable`](druid::stats::DataSourceMonitorable) instances on a configurable interval, coalesces
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
pub(crate) mod command_journal;
pub mod command_payload;
pub mod config;
pub mod error;
#[cfg(feature = "client")]
mod grpc_authorization;
#[cfg(feature = "client")]
pub mod grpc_client;
#[cfg(feature = "client")]
pub mod grpc_client_config_builder;
pub mod ingest_handler;
pub(crate) mod pipeline_ownership;
pub mod prometheus;
pub mod protocol;
pub mod push_worker;
pub mod registry;
pub mod runtime;
pub mod sampler;
pub mod sanitizer;
pub mod self_metrics;
pub mod sequence_window;
mod service_identity;
pub(crate) mod snapshot_encoder;
pub(crate) mod snapshot_transaction_state;
pub mod timeline;

pub use config::{DruidMetricsConfig, DruidMetricsConfigBuilder, SqlTextPolicy};
pub use error::{MetricsConfigError, MetricsError};
pub use registry::RegistrationGuard;
pub use runtime::DruidMetricsRuntime;
pub use service_identity::{
    ServiceIdentity, MAX_CANONICAL_SERVICE_ID_BYTES, MAX_SERVICE_ID_COMPONENT_BYTES,
};
