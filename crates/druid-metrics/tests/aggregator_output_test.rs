use std::sync::Arc;
use std::time::Duration;

use druid::stats::{
    DataSourceIdentity, DruidTelemetrySnapshot, PoolSnapshot, SqlMerger, SqlStatSnapshot,
    WallSnapshot,
};
use druid_metrics::DruidMetricsConfig;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod config {
    pub use druid_metrics::config::*;
}

mod self_metrics {
    pub use druid_metrics::self_metrics::*;
}

mod error {
    pub use druid_metrics::error::*;
}

mod protocol {
    pub use druid_metrics::protocol::*;
}

#[path = "../src/snapshot_encoder.rs"]
mod snapshot_encoder;

#[path = "../src/aggregator.rs"]
mod aggregator_under_test;

use aggregator_under_test::{AggregatedBatch, AggregatorCommand, PendingSnapshot};
use config::SqlTextPolicy;
use error::MetricsError;
use self_metrics::RuntimeSelfMetrics;

fn snapshot(datasource_id: u64, sampling_time_millis: u64) -> PendingSnapshot {
    PendingSnapshot {
        datasource_id,
        snapshot: DruidTelemetrySnapshot {
            identity: DataSourceIdentity {
                id: datasource_id,
                name: format!("datasource-{datasource_id}"),
                driver_name: Some("test-driver".to_owned()),
            },
            pool_snapshot: PoolSnapshot {
                active_count: 1,
                idle_count: 2,
                max_active: 8,
                max_idle: 4,
                waiting_count: 0,
            },
            sql_stats: Vec::new(),
            wall_snapshot: Some(WallSnapshot {
                check_count: 9,
                deny_count: 1,
                violation_count: 2,
            }),
            sampling_time_millis,
        },
    }
}

fn config(batch_size: usize, coalesce_window: Duration) -> DruidMetricsConfig {
    DruidMetricsConfig::builder()
        .queue_capacity(8)
        .batch_size(batch_size)
        .coalesce_window(coalesce_window)
        .build()
        .expect("test config is valid")
}

type AggregatorHarness = (
    mpsc::Sender<PendingSnapshot>,
    mpsc::Receiver<AggregatedBatch>,
    mpsc::Sender<AggregatorCommand>,
    CancellationToken,
    tokio::task::JoinHandle<Result<(), MetricsError>>,
);

fn spawn_aggregator(config: DruidMetricsConfig) -> AggregatorHarness {
    let (snapshot_tx, snapshot_rx) = mpsc::channel(config.queue_capacity);
    let (batch_tx, batch_rx) = mpsc::channel(config.queue_capacity);
    let (command_tx, command_rx) = mpsc::channel(1);
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let task = tokio::spawn(async move {
        aggregator_under_test::run_aggregator(
            snapshot_rx,
            batch_tx,
            command_rx,
            task_cancel,
            config,
            Arc::new(RuntimeSelfMetrics::new()),
        )
        .await
    });

    (snapshot_tx, batch_rx, command_tx, cancel, task)
}

fn flush_command() -> (
    AggregatorCommand,
    tokio::sync::oneshot::Receiver<Result<(), MetricsError>>,
) {
    let (completion, receiver) = tokio::sync::oneshot::channel();
    (AggregatorCommand::Flush { completion }, receiver)
}

#[tokio::test(start_paused = true)]
async fn same_datasource_keeps_newest_snapshot_until_window_expires() {
    let window = Duration::from_millis(500);
    let (snapshot_tx, mut batch_rx, _command_tx, cancel, task) =
        spawn_aggregator(config(8, window));

    snapshot_tx.send(snapshot(7, 10)).await.unwrap();
    snapshot_tx.send(snapshot(7, 11)).await.unwrap();
    tokio::task::yield_now().await;

    assert!(batch_rx.try_recv().is_err());
    tokio::time::advance(window).await;
    tokio::task::yield_now().await;

    let batch = batch_rx.recv().await.expect("window flush emits a batch");
    assert_eq!(batch.entries.len(), 1);
    assert_eq!(batch.entries[0].datasource_id, 7);
    assert_eq!(batch.entries[0].sampling_time_millis, 11);
    assert!(!batch.is_full_snapshot);

    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn batch_size_plus_one_distinct_datasources_split_into_two_batches() {
    let window = Duration::from_secs(1);
    let (snapshot_tx, mut batch_rx, _command_tx, cancel, task) =
        spawn_aggregator(config(2, window));

    for datasource_id in 1..=3 {
        snapshot_tx
            .send(snapshot(datasource_id, datasource_id))
            .await
            .unwrap();
    }
    tokio::task::yield_now().await;
    tokio::time::advance(window).await;
    tokio::task::yield_now().await;

    let first = batch_rx.recv().await.expect("full batch is emitted");
    let second = batch_rx.recv().await.expect("window emits the remainder");
    assert_eq!(first.entries.len(), 2);
    assert_eq!(second.entries.len(), 1);
    assert_eq!(
        first.entries.len() + second.entries.len(),
        3,
        "no datasource may be lost at the batch boundary"
    );

    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn explicit_flush_emits_partial_non_full_batch() {
    let (snapshot_tx, mut batch_rx, command_tx, cancel, task) =
        spawn_aggregator(config(8, Duration::from_secs(30)));

    snapshot_tx.send(snapshot(3, 20)).await.unwrap();
    let (command, completion) = flush_command();
    command_tx.send(command).await.unwrap();

    let batch = batch_rx.recv().await.expect("flush emits pending entries");
    assert_eq!(batch.entries.len(), 1);
    assert!(!batch.is_full_snapshot);
    completion.await.unwrap().unwrap();

    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn explicit_flush_drains_snapshots_accepted_before_the_command() {
    let (snapshot_tx, mut batch_rx, command_tx, cancel, task) =
        spawn_aggregator(config(8, Duration::from_secs(30)));

    snapshot_tx.send(snapshot(31, 310)).await.unwrap();
    let (command, completion) = flush_command();
    command_tx.send(command).await.unwrap();

    let batch = tokio::time::timeout(Duration::from_millis(1), batch_rx.recv())
        .await
        .expect("flush must not wait for the coalesce window")
        .expect("flush emits accepted snapshots");
    assert_eq!(batch.entries.len(), 1);
    assert_eq!(batch.entries[0].datasource_id, 31);
    completion.await.unwrap().unwrap();

    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn explicit_flush_completion_acknowledges_successful_output() {
    let (snapshot_tx, mut batch_rx, command_tx, cancel, task) =
        spawn_aggregator(config(8, Duration::from_secs(30)));
    snapshot_tx.send(snapshot(32, 320)).await.unwrap();
    let (command, completion_rx) = flush_command();

    command_tx.send(command).await.unwrap();

    let completion = tokio::time::timeout(Duration::from_millis(1), completion_rx)
        .await
        .expect("flush completion must not wait for the coalesce window")
        .expect("aggregator must answer the flush command");
    assert!(completion.is_ok(), "successful output must be acknowledged");
    assert_eq!(batch_rx.recv().await.unwrap().entries[0].datasource_id, 32);

    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn expired_window_flushes_while_snapshot_input_remains_ready() {
    let window = Duration::from_millis(20);
    let (snapshot_tx, mut batch_rx, _command_tx, cancel, task) =
        spawn_aggregator(config(100_000, window));
    snapshot_tx.send(snapshot(41, 1)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(1)).await;

    for sampling_time in 2..=9 {
        snapshot_tx.try_send(snapshot(41, sampling_time)).unwrap();
    }
    // Freeze the single-thread test runtime until the deadline has elapsed.
    // When scheduling resumes both the timer and snapshot receiver are ready.
    std::thread::sleep(window + Duration::from_millis(10));

    let batch = batch_rx.recv().await.expect("expired window emits a batch");
    assert_eq!(batch.entries.len(), 1);
    assert_eq!(batch.entries[0].datasource_id, 41);
    assert!(
        batch.entries[0].sampling_time_millis < 9,
        "expired deadline must flush before draining all already-ready input"
    );

    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn shutdown_drains_queued_snapshots_before_closing_output() {
    let (snapshot_tx, mut batch_rx, _command_tx, cancel, task) =
        spawn_aggregator(config(8, Duration::from_secs(30)));

    snapshot_tx.send(snapshot(4, 30)).await.unwrap();
    cancel.cancel();
    task.await.unwrap().unwrap();

    let batch = batch_rx.recv().await.expect("shutdown flushes queued work");
    assert_eq!(batch.entries.len(), 1);
    assert!(!batch.is_full_snapshot);
    assert!(batch_rx.recv().await.is_none(), "output closes after flush");
}

#[tokio::test(start_paused = true)]
async fn input_close_flushes_exact_chunks_and_closes_output() {
    let (snapshot_tx, mut batch_rx, _command_tx, _cancel, task) =
        spawn_aggregator(config(2, Duration::from_secs(30)));
    for datasource_id in 1..=5 {
        snapshot_tx
            .try_send(snapshot(datasource_id, datasource_id))
            .unwrap();
    }
    drop(snapshot_tx);

    task.await.unwrap().unwrap();

    let mut sizes = Vec::new();
    while let Some(batch) = batch_rx.recv().await {
        sizes.push(batch.entries.len());
    }
    assert_eq!(sizes, vec![2, 2, 1]);
}

#[tokio::test(start_paused = true)]
async fn cancellation_flushes_exact_chunks_and_closes_output() {
    let (snapshot_tx, mut batch_rx, _command_tx, cancel, task) =
        spawn_aggregator(config(2, Duration::from_secs(30)));
    for datasource_id in 1..=5 {
        snapshot_tx
            .try_send(snapshot(datasource_id, datasource_id))
            .unwrap();
    }
    cancel.cancel();

    task.await.unwrap().unwrap();

    let mut sizes = Vec::new();
    while let Some(batch) = batch_rx.recv().await {
        sizes.push(batch.entries.len());
    }
    assert_eq!(sizes, vec![2, 2, 1]);
}

#[tokio::test(start_paused = true)]
async fn cancellation_reports_final_flush_failure_when_output_is_closed() {
    let config = config(8, Duration::from_secs(30));
    let (snapshot_tx, snapshot_rx) = mpsc::channel(config.queue_capacity);
    let (batch_tx, batch_rx) = mpsc::channel(1);
    let (_command_tx, command_rx) = mpsc::channel(1);
    drop(batch_rx);
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let metrics = Arc::new(RuntimeSelfMetrics::new());
    let task_metrics = Arc::clone(&metrics);
    let task = tokio::spawn(async move {
        aggregator_under_test::run_aggregator(
            snapshot_rx,
            batch_tx,
            command_rx,
            task_cancel,
            config,
            task_metrics,
        )
        .await
    });
    snapshot_tx.try_send(snapshot(51, 510)).unwrap();
    cancel.cancel();

    let error = task.await.unwrap().unwrap_err();

    assert!(matches!(error, MetricsError::ChannelSend(_)));
    assert_eq!(metrics.pending_snapshots(), 1);
}

#[tokio::test(start_paused = true)]
async fn full_output_channel_backpressures_without_dropping_the_next_batch() {
    let config = config(1, Duration::from_secs(30));
    let (snapshot_tx, snapshot_rx) = mpsc::channel(config.queue_capacity);
    let (batch_tx, mut batch_rx) = mpsc::channel(1);
    let (_command_tx, command_rx) = mpsc::channel(1);
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let metrics = Arc::new(RuntimeSelfMetrics::new());
    let task_metrics = Arc::clone(&metrics);
    let task = tokio::spawn(async move {
        aggregator_under_test::run_aggregator(
            snapshot_rx,
            batch_tx,
            command_rx,
            task_cancel,
            config,
            task_metrics,
        )
        .await
    });

    snapshot_tx.send(snapshot(1, 1)).await.unwrap();
    snapshot_tx.send(snapshot(2, 2)).await.unwrap();
    tokio::task::yield_now().await;

    assert_eq!(metrics.pending_snapshots(), 1);
    let first = batch_rx.recv().await.expect("first batch fills output");
    let second = batch_rx
        .recv()
        .await
        .expect("draining capacity releases the blocked second batch");
    assert_eq!(first.entries[0].datasource_id, 1);
    assert_eq!(second.entries[0].datasource_id, 2);

    cancel.cancel();
    task.await.unwrap().unwrap();
}

#[tokio::test(start_paused = true)]
async fn closed_output_channel_stops_aggregation_and_retains_unflushed_count() {
    let config = config(8, Duration::from_secs(30));
    let (snapshot_tx, snapshot_rx) = mpsc::channel(config.queue_capacity);
    let (batch_tx, batch_rx) = mpsc::channel(1);
    let (command_tx, command_rx) = mpsc::channel(1);
    drop(batch_rx);
    let cancel = CancellationToken::new();
    let metrics = Arc::new(RuntimeSelfMetrics::new());
    let task_metrics = Arc::clone(&metrics);
    let task = tokio::spawn(async move {
        aggregator_under_test::run_aggregator(
            snapshot_rx,
            batch_tx,
            command_rx,
            cancel,
            config,
            task_metrics,
        )
        .await
    });

    snapshot_tx.send(snapshot(5, 5)).await.unwrap();
    tokio::task::yield_now().await;
    let (command, completion) = flush_command();
    command_tx.send(command).await.unwrap();
    tokio::task::yield_now().await;

    assert!(task.is_finished(), "closed output must stop the aggregator");
    let error = task.await.unwrap().unwrap_err();
    assert!(matches!(error, MetricsError::ChannelSend(_)));
    assert!(matches!(
        completion.await.unwrap(),
        Err(MetricsError::ChannelSend(_))
    ));
    assert_eq!(metrics.pending_snapshots(), 1);
}

#[test]
fn typed_encoder_limits_sql_and_applies_disabled_privacy_policy() {
    let mut pending = snapshot(9, 40);
    pending.snapshot.sql_stats = vec![
        SqlStatSnapshot {
            fingerprint: "SELECT * FROM account WHERE password = ?".to_owned(),
            exec_count: 10,
            exec_time_millis: 11,
            fetch_row_count: 12,
            update_count: 13,
        },
        SqlStatSnapshot {
            fingerprint: "SELECT * FROM audit WHERE token = ?".to_owned(),
            exec_count: 20,
            exec_time_millis: 21,
            fetch_row_count: 22,
            update_count: 23,
        },
    ];

    let entry = snapshot_encoder::encode_snapshot(pending.snapshot, 1, SqlTextPolicy::Disabled);

    assert_eq!(entry.sql_stats.len(), 1);
    assert_eq!(entry.sql_stats[0].fingerprint, "");
    assert_eq!(entry.sql_stats[0].exec_count, 10);
    let encoded = prost::Message::encode_to_vec(&entry);
    let encoded_text = String::from_utf8_lossy(&encoded);
    assert!(!encoded_text.contains("password"));
    assert!(!encoded_text.contains("token"));
}

#[test]
fn typed_encoder_maps_pool_wall_and_identity_fields_without_json() {
    let pending = snapshot(12, 60);

    let entry =
        snapshot_encoder::encode_snapshot(pending.snapshot, 8, SqlTextPolicy::RawWithoutParameters);

    assert_eq!(entry.datasource_id, 12);
    assert_eq!(entry.datasource_name, "datasource-12");
    assert_eq!(entry.driver_name.as_deref(), Some("test-driver"));
    let pool = entry.pool.expect("pool snapshot is typed and present");
    assert_eq!(pool.active_count, 1);
    assert_eq!(pool.idle_count, 2);
    assert_eq!(pool.max_active, 8);
    assert_eq!(pool.max_idle, 4);
    assert_eq!(pool.waiting_count, 0);
    assert_eq!(entry.wall_check_count, 9);
    assert_eq!(entry.wall_deny_count, 1);
    assert_eq!(entry.wall_violation_count, 2);
    assert_eq!(entry.sampling_time_millis, 60);
}

#[test]
fn typed_encoder_exports_opaque_identity_even_for_parameterized_input() {
    let mut pending = snapshot(10, 50);
    pending.snapshot.sql_stats = vec![SqlStatSnapshot {
        fingerprint: "SELECT * FROM customer WHERE id = ?".to_owned(),
        exec_count: 4,
        exec_time_millis: 5,
        fetch_row_count: 6,
        update_count: 7,
    }];

    let entry =
        snapshot_encoder::encode_snapshot(pending.snapshot, 8, SqlTextPolicy::FingerprintOnly);

    assert_eq!(entry.sql_stats.len(), 1);
    assert_eq!(
        entry.sql_stats[0].fingerprint,
        "sha256:1549035f7d86ffa43cffc1a007df135f5e097447755a101ed4278510e534ecfe"
    );
    assert!(!entry.sql_stats[0].fingerprint.contains("SELECT"));
}

#[test]
fn real_unmerged_sql_never_reaches_protocol_or_debug_output() {
    const RAW_SQL: &str = "SELECT * FROM customer WHERE id = 420042 AND name = \
        'top_secret_customer' /* tenant-secret-comment */";
    const FORBIDDEN: &[&str] = &[
        RAW_SQL,
        "420042",
        "top_secret_customer",
        "tenant-secret-comment",
    ];

    let merger = SqlMerger::new();
    let stat = merger.record_with_merge_stat(RAW_SQL, Duration::from_millis(7), true, false);
    let value = stat.stat_value();
    assert_eq!(value.sql, RAW_SQL, "fixture must exercise merge_sql=false");

    for policy in [
        SqlTextPolicy::FingerprintOnly,
        SqlTextPolicy::RawWithoutParameters,
    ] {
        let mut pending = snapshot(13, 70);
        pending.snapshot.sql_stats = vec![SqlStatSnapshot {
            fingerprint: value.sql.clone(),
            exec_count: value.execute_count,
            exec_time_millis: value.total_time_millis,
            fetch_row_count: value.fetch_row_count,
            update_count: value.update_count,
        }];
        let entry = snapshot_encoder::encode_snapshot(pending.snapshot, 8, policy);
        let encoded = prost::Message::encode_to_vec(&entry);
        let debug = format!("{entry:?}");

        assert!(entry.sql_stats[0].fingerprint.starts_with("sha256:"));
        for forbidden in FORBIDDEN {
            assert!(
                !encoded
                    .windows(forbidden.len())
                    .any(|window| window == forbidden.as_bytes()),
                "encoded protocol leaked {forbidden:?} under {policy:?}"
            );
            assert!(
                !debug.contains(forbidden),
                "Debug output leaked {forbidden:?} under {policy:?}"
            );
        }
    }
}
