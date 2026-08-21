//! 接收 druid-metrics 运行时指标的 gRPC ingest 服务。

mod ingest_service;

pub use ingest_service::IngestService;
