//! gRPC ingest service for receiving metrics from druid-metrics runtime.

mod ingest_service;

pub use ingest_service::{ingest_proto, IngestService};
