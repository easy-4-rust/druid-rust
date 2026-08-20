//! In-memory metrics repository for the standalone admin.
//!
//! Stores the latest snapshot per datasource instance. Long-term history
//! is delegated to Prometheus; this repository only holds the current state.

mod metrics_repository;

pub use metrics_repository::{DataSourceEntry, DataSourceId, MetricsRepository};
