//! RED tests for Prometheus /metrics endpoint.
//!
//! Task 4 Step 1: Tests for /metrics endpoint returning Prometheus text format.

use std::collections::HashMap;
use std::time::Duration;

use druid_admin::model::dto::{DataSourceContent, WallResult};
use druid_admin::repository::{
    DataSourceEntry, DataSourceId, MetricsRepository, MetricsRepositoryConfig,
};
use druid_metrics::ServiceIdentity;

/// Helper: start the standalone HTTP server with data in the repository.
async fn start_server_with_data() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    start_server_with_datasource_count(1).await
}

async fn start_server_with_datasource_count(
    datasource_count: i64,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let repo = MetricsRepository::new();

    for identity in 1..=datasource_count {
        let service_name = format!("svc-{identity}");
        let service_identity =
            ServiceIdentity::try_new(service_name.clone(), "metrics-instance", "metrics-boot")
                .unwrap();
        let id = DataSourceId {
            service_identity: service_identity.clone(),
            identity,
        };
        let entry = DataSourceEntry {
            datasource: DataSourceContent {
                identity,
                name: Some(format!("service-{identity}")),
                service_id: Some(service_identity.shared_service_id()),
                service_name: Some(service_name.into()),
                instance_id: Some("metrics-instance".into()),
                boot_id: Some("metrics-boot".into()),
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

    start_server_with_repository(repo).await
}

async fn start_server_with_repository(
    repo: MetricsRepository,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

#[tokio::test]
async fn expired_boot_session_disappears_from_rest_and_prometheus() {
    let config = MetricsRepositoryConfig::try_new(2, 2, 2, Duration::from_secs(1), 1 << 20)
        .expect("valid TTL budget");
    let repository = MetricsRepository::with_config(config);
    let identity = ServiceIdentity::try_new("expired-service", "pod-1", "expired-boot").unwrap();
    let id = DataSourceId {
        service_identity: identity.clone(),
        identity: 7,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity: 7,
            service_id: Some(identity.shared_service_id()),
            service_name: Some("expired-service".into()),
            instance_id: Some("pod-1".into()),
            boot_id: Some("expired-boot".into()),
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall: WallResult::default(),
        last_updated_ms: i64::MAX,
        sequence: 1,
    };
    assert!(repository.upsert_datasource(id, entry));
    let (addr, handle) = start_server_with_repository(repository).await;
    let rest_url = format!("http://{addr}/druid/api/datasources");
    let metrics_url = format!("http://{addr}/metrics");
    let mut detail_url =
        reqwest::Url::parse(&format!("http://{addr}/druid/api/datasources/7")).unwrap();
    detail_url
        .query_pairs_mut()
        .append_pair("serviceId", identity.service_id());

    let before: serde_json::Value = reqwest::get(&rest_url).await.unwrap().json().await.unwrap();
    assert_eq!(before["Content"].as_array().unwrap().len(), 1);
    assert_eq!(
        reqwest::get(detail_url.clone()).await.unwrap().status(),
        200
    );
    let before_metrics = reqwest::get(&metrics_url)
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(before_metrics.contains("druid_admin_datasource_count 1"));
    assert!(before_metrics.lines().any(|line| {
        line.strip_prefix("druid_admin_repository_state_bytes ")
            .is_some_and(|value| value.parse::<usize>().is_ok_and(|bytes| bytes > 0))
    }));

    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let after: serde_json::Value = reqwest::get(&rest_url).await.unwrap().json().await.unwrap();
    assert!(after["Content"].as_array().unwrap().is_empty());
    assert_eq!(reqwest::get(detail_url).await.unwrap().status(), 404);
    let after_metrics = reqwest::get(&metrics_url)
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(after_metrics.contains("druid_admin_datasource_count 0"));
    assert!(after_metrics.contains("druid_admin_repository_state_bytes 0"));
    assert!(!after_metrics.contains("expired-boot"));
    handle.abort();
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

#[tokio::test]
async fn metrics_output_over_budget_is_rejected_and_counted_without_silent_truncation() {
    let config = MetricsRepositoryConfig::try_new(4, 4, 4, Duration::from_secs(60), 256)
        .expect("valid metrics output budget");
    let repository = MetricsRepository::with_config(config);
    let service_identity =
        ServiceIdentity::try_new("bounded-service", "metrics-instance", "metrics-boot").unwrap();
    let id = DataSourceId {
        service_identity: service_identity.clone(),
        identity: 1,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity: 1,
            name: Some("bounded-service".to_owned()),
            service_id: Some(service_identity.shared_service_id()),
            service_name: Some("bounded-service".into()),
            instance_id: Some("metrics-instance".into()),
            boot_id: Some("metrics-boot".into()),
            db_type: Some("postgresql".to_owned()),
            active_count: 7,
            ..Default::default()
        },
        sql_stats: HashMap::new(),
        wall: WallResult::default(),
        last_updated_ms: 0,
        sequence: 1,
    };
    assert!(repository.upsert_datasource(id, entry));
    let app = druid_admin::standalone_router(repository.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind bounded metrics HTTP");
    let address = listener.local_addr().expect("bounded metrics address");
    let handle = tokio::spawn(async move { axum::serve(listener, app).await });

    let response = reqwest::get(format!("http://{address}/metrics"))
        .await
        .expect("GET bounded metrics");
    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body = response
        .text()
        .await
        .expect("bounded metrics response body");
    assert!(body.contains("druid_admin_metrics_output_rejected_total 1"));
    assert!(body.contains("druid_admin_metrics_output_limit_bytes 256"));
    assert!(
        !body.contains("druid_admin_datasource_active_count"),
        "over-budget exposition is rejected explicitly, never silently truncated"
    );
    assert_eq!(repository.limit_counters(), (0, 0, 1));
    handle.abort();
}
