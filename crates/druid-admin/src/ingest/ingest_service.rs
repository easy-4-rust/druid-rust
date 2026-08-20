//! gRPC ingest service that receives metrics pushes from druid-metrics runtime.
//!
//! Implements the `MetricsIngest` gRPC service defined in `proto/ingest.proto`.
//! Each incoming `MetricsSnapshot` is converted to a `DataSourceEntry` and
//! upserted into the shared `MetricsRepository` with sequence-based deduplication.

use tonic::{Request, Response, Status, Streaming};

use crate::model::dto::{DataSourceContent, SqlListContent, WallResult};
use crate::repository::{DataSourceEntry, DataSourceId, MetricsRepository};

// Include the generated gRPC code from proto/ingest.proto.
pub mod ingest_proto {
    tonic::include_proto!("druid.admin.ingest");
}

use ingest_proto::metrics_ingest_server::{MetricsIngest, MetricsIngestServer};
use ingest_proto::{MetricsSnapshot, PushResponse, WallStats};

/// gRPC ingest service implementation.
///
/// Receives `MetricsSnapshot` streams and upserts them into the
/// shared `MetricsRepository`.
#[derive(Clone)]
pub struct IngestService {
    repo: MetricsRepository,
}

impl IngestService {
    /// Create a new ingest service backed by the given repository.
    #[must_use]
    pub fn new(repo: MetricsRepository) -> Self {
        Self { repo }
    }

    /// Returns a reference to the underlying repository.
    pub fn repository(&self) -> &MetricsRepository {
        &self.repo
    }

    /// Create a `MetricsIngestServer` from this service for tonic.
    #[must_use]
    pub fn into_server(self) -> MetricsIngestServer<Self> {
        MetricsIngestServer::new(self)
    }
}

#[tonic::async_trait]
impl MetricsIngest for IngestService {
    async fn push_snapshots(
        &self,
        request: Request<Streaming<MetricsSnapshot>>,
    ) -> Result<Response<PushResponse>, Status> {
        let repo = self.repo.clone();
        let mut stream = request.into_inner();

        let mut accepted: i32 = 0;
        let mut rejected: i32 = 0;

        while let Some(snapshot) = stream.message().await? {
            let id = DataSourceId {
                service_id: snapshot.service_id.clone(),
                identity: snapshot.identity,
            };
            let entry = snapshot_to_entry(&snapshot);
            if repo.upsert_datasource(id, entry) {
                accepted += 1;
            } else {
                rejected += 1;
            }
        }

        Ok(Response::new(PushResponse { accepted, rejected }))
    }
}

/// Convert a gRPC `MetricsSnapshot` proto into a `DataSourceEntry`.
fn snapshot_to_entry(snapshot: &MetricsSnapshot) -> DataSourceEntry {
    let ds = snapshot.datasource.as_ref();

    DataSourceEntry {
        datasource: DataSourceContent {
            identity: snapshot.identity,
            db_type: ds.map(|d| d.db_type.clone()).filter(|s| !s.is_empty()),
            url: ds.map(|d| d.url.clone()).filter(|s| !s.is_empty()),
            user_name: ds.map(|d| d.user_name.clone()).filter(|s| !s.is_empty()),
            active_count: ds.map_or(0, |d| d.active_count),
            pooling_count: ds.map_or(0, |d| d.pooling_count),
            execute_count: ds.map_or(0, |d| d.execute_count),
            error_count: ds.map_or(0, |d| d.error_count),
            commit_count: ds.map_or(0, |d| d.commit_count),
            rollback_count: ds.map_or(0, |d| d.rollback_count),
            ..Default::default()
        },
        sql_stats: snapshot
            .sql_stats
            .iter()
            .map(|(hash, sql)| {
                (
                    *hash,
                    SqlListContent {
                        id: sql.id,
                        sql: Some(sql.sql.clone()),
                        execute_count: sql.execute_count,
                        total_time: sql.total_time,
                        max_timespan: sql.max_timespan,
                        error_count: sql.error_count,
                        running_count: sql.running_count,
                        concurrent_max: sql.concurrent_max,
                        fetch_row_count: sql.fetch_row_count,
                        effected_row_count: sql.effected_row_count,
                        db_type: Some(sql.db_type.clone()),
                        ..Default::default()
                    },
                )
            })
            .collect(),
        wall: wall_proto_to_dto(snapshot.wall.as_ref()),
        last_updated_ms: snapshot.timestamp_ms,
        sequence: snapshot.sequence,
    }
}

/// Convert gRPC `WallStats` proto into a `WallResult`.
fn wall_proto_to_dto(wall: Option<&WallStats>) -> WallResult {
    let mut result = WallResult::default();
    if let Some(w) = wall {
        result.content.check_count = w.check_count;
        result.content.hard_check_count = w.hard_check_count;
        result.content.violation_count = w.violation_count;
        result.content.syntax_error_count = w.syntax_error_count;
        result.content.black_list_hit_count = w.black_list_hit_count;
        result.content.white_list_hit_count = w.white_list_hit_count;
        result.content.black_list_size = w.black_list_size;
        result.content.white_list_size = w.white_list_size;
    }
    result
}
