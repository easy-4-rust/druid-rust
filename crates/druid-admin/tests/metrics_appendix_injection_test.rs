use std::time::Duration;
use std::{collections::HashMap, sync::Arc};

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use druid_admin::config::MonitorProperties;
use druid_admin::repository::{MetricsRepository, MetricsRepositoryConfig};
use druid_admin::service::{MonitorStatService, StaticDiscoveryClient};
use druid_admin::util::ReqwestHttpClient;
use druid_admin::MonitorViewServlet;
use tower::ServiceExt as _;

#[tokio::test]
async fn injected_metrics_are_exposed_without_driver_dependency() {
    let router = MonitorViewServlet::from_repository(MetricsRepository::new())
        .with_metrics_appendix(|| "# TYPE host_driver_ready gauge\nhost_driver_ready 1\n".into())
        .router();
    let response = router
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("metrics response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 4096).await.expect("body");
    assert!(
        String::from_utf8_lossy(&body).contains("host_driver_ready 1"),
        "host-provided metrics must be included"
    );
}

#[tokio::test]
async fn injected_metrics_obey_repository_output_budget() {
    let config = MetricsRepositoryConfig::try_new(4, 4, 4, Duration::from_secs(60), 256)
        .expect("valid budget");
    let repository = MetricsRepository::with_config(config);
    let router = MonitorViewServlet::from_repository(repository.clone())
        .with_metrics_appendix(|| "x".repeat(512))
        .router();
    let response = router
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("metrics response");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), 4096).await.expect("body");
    assert!(String::from_utf8_lossy(&body).contains("druid_admin_metrics_output_rejected_total 1"));
    assert_eq!(repository.limit_counters(), (0, 0, 1));
}

#[tokio::test]
async fn remote_metrics_appendix_cannot_bypass_default_output_budget() {
    let service = Arc::new(MonitorStatService::new(
        Arc::new(StaticDiscoveryClient::new(HashMap::new())),
        Arc::new(MonitorProperties::default()),
        None,
        Arc::new(
            ReqwestHttpClient::new(Duration::from_secs(1), Duration::from_secs(1))
                .expect("HTTP client"),
        ),
    ));
    let router = MonitorViewServlet::new(service)
        .with_metrics_appendix(|| "x".repeat(1 << 20))
        .router();
    for expected_rejections in 1..=2 {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("metrics response");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = to_bytes(response.into_body(), 4096).await.expect("body");
        assert!(String::from_utf8_lossy(&body).contains(&format!(
            "druid_admin_metrics_output_rejected_total {expected_rejections}"
        )));
    }
}
