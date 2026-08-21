//! 共享 druid-metrics ingest 协议的编译合同。

use druid_metrics::protocol::metrics_ingest_server::{MetricsIngest, MetricsIngestServer};
use druid_metrics::protocol::{ClientFrame, ServerFrame};
use tonic::{Request, Response, Status, Streaming};

/// 仅用于证明 Admin 能实现并构造共享生成服务端。
#[derive(Clone)]
struct TestService;

#[tonic::async_trait]
impl MetricsIngest for TestService {
    type ConnectStream = tokio_stream::Empty<Result<ServerFrame, Status>>;

    async fn connect(
        &self,
        _request: Request<Streaming<ClientFrame>>,
    ) -> Result<Response<Self::ConnectStream>, Status> {
        Ok(Response::new(tokio_stream::empty()))
    }
}

#[test]
fn admin_builds_server_from_druid_metrics_protocol() {
    let _server = MetricsIngestServer::new(TestService);
}
