//! 共享 druid-metrics 协议的 gRPC ingest 服务。

use druid_metrics::protocol::metrics_ingest_server::{MetricsIngest, MetricsIngestServer};
use druid_metrics::protocol::{ClientFrame, ServerFrame};
use tonic::{Request, Response, Status, Streaming};

use crate::repository::MetricsRepository;

/// gRPC ingest 服务实现。
///
/// Task 1 仅冻结生成的服务端合同。帧处理与 ACK 由后续运行时任务接入，
/// 在真实实现完成前该服务明确返回未实现错误。
#[derive(Clone)]
pub struct IngestService {
    repo: MetricsRepository,
}

impl IngestService {
    /// 使用指定指标仓库创建 ingest 服务。
    #[must_use]
    pub fn new(repo: MetricsRepository) -> Self {
        Self { repo }
    }

    /// 返回底层指标仓库引用。
    pub fn repository(&self) -> &MetricsRepository {
        &self.repo
    }

    /// 将当前服务包装为 Tonic [`MetricsIngestServer`]。
    #[must_use]
    pub fn into_server(self) -> MetricsIngestServer<Self> {
        MetricsIngestServer::new(self)
    }
}

#[tonic::async_trait]
impl MetricsIngest for IngestService {
    type ConnectStream = tokio_stream::Empty<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        _request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        Err(Status::unimplemented(
            "shared metrics ingest runtime is not wired yet",
        ))
    }
}
