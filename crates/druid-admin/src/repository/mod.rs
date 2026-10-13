//! In-memory metrics repository for the standalone admin.
//!
//! Stores the latest snapshot per datasource instance. Long-term history
//! is delegated to Prometheus; this repository only holds the current state.

mod metrics_repository;
mod metrics_repository_config;
#[cfg(test)]
mod metrics_repository_test;
mod serialized_size_budget;
mod stored_data_source_entry;

pub use metrics_repository::{DataSourceEntry, DataSourceId, MetricsRepository};
pub use metrics_repository_config::MetricsRepositoryConfig;
pub(crate) use serialized_size_budget::SerializedSizeBudget;
pub(crate) use stored_data_source_entry::StoredDataSourceEntry;
