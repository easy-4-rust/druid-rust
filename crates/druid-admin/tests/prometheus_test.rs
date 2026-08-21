//! RED tests for Prometheus /metrics endpoint.
//!
//! Task 4 Step 1: Tests for /metrics endpoint returning Prometheus text format.

use std::collections::HashMap;
use std::time::Duration;

use druid_admin::model::dto::{DataSourceContent, WallResult};
use druid_admin::repository::{DataSourceEntry, DataSourceId, MetricsRepository};

/// Helper: start the standalone HTTP server with data in the repository.
async fn start_server_with_data() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    start_server_with_datasource_count(1).await
}

async fn start_server_with_datasource_count(
    datasource_count: i64,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let repo = MetricsRepository::new();

    for identity in 1..=datasource_count {
        let id = DataSourceId {
            service_id: format!("svc-{identity}"),
            identity,
        };
        let entry = DataSourceEntry {
            datasource: DataSourceContent {
                identity,
                name: Some(format!("service-{identity}")),
                db_type: Some("mysql".to_owned()),
                active_count: 5 + identity,
                pooling_count: 10 + identity,
                execute_count: 1000 + identity,
                error_count: identity,
                commit_count: identity,
                rollback_count: identity,
                ..Default::default()
            },
            sql_stats: HashMap::new(),
            wall: WallResult::default(),
            last_updated_ms: 0,
            sequence: 1,
        };
        assert!(repo.upsert_datasource(id, entry));
    }

    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

/// /metrics must return 200 OK.
#[tokio::test]
async fn metrics_returns_ok() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    assert_eq!(resp.status(), 200);

    handle.abort();
}

/// /metrics must return Content-Type: text/plain; version=0.0.4 (Prometheus text format).
#[tokio::test]
async fn metrics_content_type_is_prometheus() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.contains("text/plain"),
        "Content-Type must be text/plain, got: {content_type}"
    );

    handle.abort();
}

/// /metrics must contain `druid_admin_datasource_active_count` metric.
#[tokio::test]
async fn metrics_contains_datasource_metrics() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(
        body.contains("druid_admin_datasource_active_count"),
        "must contain datasource active count metric:\n{body}"
    );

    handle.abort();
}

/// /metrics must contain HELP and TYPE lines for metric families.
#[tokio::test]
async fn metrics_contains_help_and_type() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(body.contains("# HELP"), "must contain HELP lines:\n{body}");
    assert!(body.contains("# TYPE"), "must contain TYPE lines:\n{body}");

    handle.abort();
}

/// /metrics must contain ingest counters from the repository.
#[tokio::test]
async fn metrics_contains_ingest_counters() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(
        body.contains("druid_admin_ingest_total"),
        "must contain ingest total counter:\n{body}"
    );
    assert!(
        body.contains("druid_admin_ingest_rejected_total"),
        "must contain ingest rejected counter:\n{body}"
    );

    handle.abort();
}

/// /metrics must contain datasource count gauge.
#[tokio::test]
async fn metrics_contains_datasource_count() {
    let (addr, handle) = start_server_with_data().await;

    let resp = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics must succeed");

    let body = resp.text().await.expect("must read body");
    assert!(
        body.contains("druid_admin_datasource_count"),
        "must contain datasource count gauge:\n{body}"
    );

    handle.abort();
}

#[tokio::test]
async fn two_datasources_emit_one_help_and_type_per_metric_family() {
    let (addr, handle) = start_server_with_datasource_count(2).await;
    let body = reqwest::get(format!("http://{addr}/metrics"))
        .await
        .expect("GET /metrics")
        .text()
        .await
        .expect("metrics text");
    let families = vec![
        ("druid_admin_tasks_instrumented_total", false),
        ("druid_admin_tasks_completed_total", false),
        ("druid_admin_task_polls_total", false),
        ("druid_admin_task_slow_polls_total", false),
        ("druid_admin_ingest_total", false),
        ("druid_admin_ingest_rejected_total", false),
        ("druid_admin_datasource_count", false),
        ("druid_admin_datasource_active_count", true),
        ("druid_admin_datasource_pooling_count", true),
        ("druid_admin_datasource_execute_count", true),
        ("druid_admin_datasource_error_count", true),
        ("druid_admin_datasource_commit_count", true),
        ("druid_admin_datasource_rollback_count", true),
    ];
    #[cfg(feature = "jdbc-agent")]
    let families = {
        let mut families = families;
        families.extend([
            ("druid_jdbc_agent_processes", false),
            ("druid_jdbc_agent_active_sessions", false),
            ("druid_jdbc_agent_starts_total", false),
            ("druid_jdbc_agent_crashes_total", false),
            ("druid_jdbc_agent_rpc_total", false),
            ("druid_jdbc_agent_rpc_errors_total", false),
            ("druid_jdbc_agent_rpc_latency_microseconds_total", false),
            ("druid_jdbc_agent_rpc_latency_microseconds_max", false),
            ("druid_jdbc_agent_timeouts_total", false),
            ("druid_jdbc_agent_cancellations_total", false),
            ("druid_jdbc_agent_protocol_errors_total", false),
        ]);
        families
    };
    for (family, per_datasource) in families {
        assert_eq!(
            body.lines()
                .filter(|line| *line == format!("# HELP {family} {}", help_text(family)))
                .count(),
            1,
            "one HELP declaration for {family}:\n{body}"
        );
        assert_eq!(
            body.lines()
                .filter(|line| line.starts_with(&format!("# TYPE {family} ")))
                .count(),
            1,
            "one TYPE declaration for {family}:\n{body}"
        );
        let sample_prefix = if per_datasource {
            format!("{family}{{")
        } else {
            format!("{family} ")
        };
        assert_eq!(
            body.lines()
                .filter(|line| line.starts_with(&sample_prefix))
                .count(),
            if per_datasource { 2 } else { 1 },
            "expected sample count for {family}:\n{body}"
        );
    }
    handle.abort();
}

fn help_text(family: &str) -> &'static str {
    match family {
        "druid_admin_tasks_instrumented_total" => "Total instrumented admin tasks.",
        "druid_admin_tasks_completed_total" => "Total completed admin tasks.",
        "druid_admin_task_polls_total" => "Total admin task polls.",
        "druid_admin_task_slow_polls_total" => "Total slow admin task polls.",
        "druid_admin_ingest_total" => "Total number of ingest batches received.",
        "druid_admin_ingest_rejected_total" => "Total number of rejected ingest batches.",
        "druid_admin_datasource_count" => "Number of tracked datasource instances.",
        "druid_admin_datasource_active_count" => "Active connection count.",
        "druid_admin_datasource_pooling_count" => "Pooled connection count.",
        "druid_admin_datasource_execute_count" => "Total execute count.",
        "druid_admin_datasource_error_count" => "Total error count.",
        "druid_admin_datasource_commit_count" => "Total commit count.",
        "druid_admin_datasource_rollback_count" => "Total rollback count.",
        "druid_jdbc_agent_processes" => "Current JDBC agent process count.",
        "druid_jdbc_agent_active_sessions" => "Current JDBC agent active sessions.",
        "druid_jdbc_agent_starts_total" => "Total JDBC agent starts.",
        "druid_jdbc_agent_crashes_total" => "Total JDBC agent crashes.",
        "druid_jdbc_agent_rpc_total" => "Total JDBC agent RPC calls.",
        "druid_jdbc_agent_rpc_errors_total" => "Total JDBC agent RPC errors.",
        "druid_jdbc_agent_rpc_latency_microseconds_total" => {
            "Total JDBC agent RPC latency in microseconds."
        }
        "druid_jdbc_agent_rpc_latency_microseconds_max" => {
            "Maximum JDBC agent RPC latency in microseconds."
        }
        "druid_jdbc_agent_timeouts_total" => "Total JDBC agent RPC timeouts.",
        "druid_jdbc_agent_cancellations_total" => "Total JDBC agent RPC cancellations.",
        "druid_jdbc_agent_protocol_errors_total" => "Total JDBC agent protocol errors.",
        _ => "",
    }
}
