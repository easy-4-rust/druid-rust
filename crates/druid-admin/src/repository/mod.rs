//! In-memory metrics repository for the standalone admin.
//!
//! 保存每个 datasource 实例的最新投影与短期有界数值采样。
//! 短期采样仅在当前 Admin 进程内，长期历史仍交由外部时序存储。

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
