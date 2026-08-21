//! In-memory metrics repository for the standalone admin.
//!
//! Stores the latest snapshot per datasource instance. Long-term history
//! is delegated to Prometheus; this repository only holds the current state.

mod metrics_repository;
mod metrics_repository_config;
mod stored_data_source_entry;

pub use metrics_repository::{DataSourceEntry, DataSourceId, MetricsRepository};
pub use metrics_repository_config::MetricsRepositoryConfig;
pub(crate) use stored_data_source_entry::StoredDataSourceEntry;
