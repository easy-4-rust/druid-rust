//! 真实控制台的可用性、身份、分页与采集健康合同（Rust 扩展）。
mod common;

use druid_admin::repository::MetricsRepository;
use druid_metrics::ServiceIdentity;
use serde_json::{json, Value};

async fn server(repo: MetricsRepository) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = druid_admin::MonitorViewServlet::from_repository(repo).router();
    let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}/druid/api/v1"), handle)
}

fn repository() -> MetricsRepository {
    let repo = MetricsRepository::new();
    for (service, count) in [("orders", 10), ("billing", 30)] {
        let (mut id, mut entry) =
            common::make_datasource_entry_with_sql(1, "sqlite", 1, 11, "fp:abc", count);
        let identity = ServiceIdentity::try_new(service, "i-1", "boot-1").unwrap();
        id.service_identity = identity;
        entry.datasource.active_count = count;
        entry.datasource.max_active = 64;
        repo.upsert_datasource(id, entry);
    }
    repo
}

#[tokio::test]
async fn push_capabilities_keep_real_values_and_mask_unreported_defaults() {
    let (base, task) = server(repository()).await;
    let body: Value = reqwest::get(format!("{base}/datasources"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let row = &body["datasources"][0];
    assert_eq!(row["source"], "push");
    assert!(row["unavailableFields"]
        .as_array()
        .unwrap()
        .contains(&json!("errorCount")));
    assert!(row["serviceId"].is_string());
    let sql: Value = reqwest::get(format!("{base}/sql"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let row = &sql["rows"][0];
    let fields = row["unavailableFields"].as_array().unwrap();
    assert!(
        !fields.contains(&json!("sql")),
        "a stored SQL text is real data even when the transport is push"
    );
    assert!(row["sql"].is_string());
    assert!(
        fields.contains(&json!("maxTimespan")),
        "a default-zero field the protocol cannot report stays masked"
    );
    task.abort();
}

#[tokio::test]
async fn colliding_ids_require_source_context() {
    let (base, task) = server(repository()).await;
    assert_eq!(
        reqwest::get(format!("{base}/datasources/1"))
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        reqwest::get(format!("{base}/sql/1"))
            .await
            .unwrap()
            .status(),
        409
    );
    let body: Value = reqwest::get(format!("{base}/datasources"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let row = body["datasources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["serviceName"] == "billing")
        .unwrap();
    let response: Value = reqwest::Client::new()
        .get(format!("{base}/datasources/1"))
        .query(&[("serviceId", row["serviceId"].as_str().unwrap())])
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(response["datasource"]["activeCount"], 30);
    task.abort();
}

#[tokio::test]
async fn health_uses_repository_counters_and_sampling_metadata() {
    let repo = repository();
    let (base, task) = server(repo).await;
    let response = reqwest::get(format!("{base}/health")).await.unwrap();
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["repository"]["ingestCount"], 2);
    assert_eq!(body["sources"].as_array().unwrap().len(), 2);
    assert_eq!(body["sources"][0]["sequence"], 1);
    assert_eq!(body["sources"][0]["samplingTimeMillis"], 1000);
    let rss_available = body["processRssBytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0);
    assert!(rss_available || body["processRssBytes"].is_null());
    let unavailable = body["unavailableFields"].as_array().unwrap();
    assert_eq!(
        unavailable.iter().any(|field| field == "processRssBytes"),
        !rss_available
    );
    task.abort();
}

#[tokio::test]
async fn sql_filters_before_bounded_pagination_and_unknown_max_is_not_slow() {
    let (base, task) = server(repository()).await;
    let body: Value = reqwest::get(format!(
        "{base}/sql?serviceName=billing&perPageCount=100000"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(body["total"], 1);
    assert_eq!(body["perPageCount"], 500);
    assert_eq!(body["rows"][0]["executeCount"], 30);
    let slow: Value = reqwest::get(format!("{base}/sql?minMaxTimespan=1"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(slow["total"], 0);
    task.abort();
}

#[tokio::test]
async fn push_connection_details_report_unsupported_instead_of_empty_success() {
    let (base, task) = server(repository()).await;
    let known: Value = reqwest::get(format!(
        "{base}/datasources/1/connections?serviceName=billing"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(known["supported"], false);
    assert!(known["rows"].is_null());
    let response = reqwest::get(format!("{base}/datasources/999/connections"))
        .await
        .unwrap();
    assert_eq!(response.status(), 404);
    task.abort();
}
#[tokio::test]
async fn poll_preserves_reported_zero_and_filters_and_reads_existing_stack() {
    use axum::{routing::get, Json, Router};
    use druid_admin::config::{AdminDiscoveryConfig, AdminDiscoveryMode};
    let node = Router::new()
        .route("/druid/datasource.json", get(|| async { Json(json!({"ResultCode":1,"Content":[{"Identity":1,"ActiveCount":0,"Name":"real-node"}]})) }))
        .route("/druid/sql.json", get(|| async { Json(json!({"ResultCode":1,"Content":[
            {"ID":7,"DataSource":1,"SQL":"select * from orders","ExecuteCount":8,"MaxTimespan":1200,"ErrorCount":0},
            {"ID":8,"DataSource":1,"SQL":"select * from products","ExecuteCount":2,"MaxTimespan":12}
        ]})) }))
        .route("/druid/activeConnectionStackTrace-1.json",get(|| async { Json(json!({"ResultCode":1,"Content":["checkout -> query"]})) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let node_task = tokio::spawn(async move { axum::serve(listener, node).await.unwrap() });
    let config = AdminDiscoveryConfig {
        mode: AdminDiscoveryMode::Poll,
        applications: vec!["orders".into()],
        endpoints: vec![format!("http://{addr}")],
    };
    let source = config
        .build_management_source(MetricsRepository::new())
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = druid_admin::MonitorViewServlet::from_management_source(source).router();
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let base = format!("http://{address}/druid/api/v1");
    let ds: Value = reqwest::get(format!("{base}/datasources"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ds["partial"], false);
    assert_eq!(ds["datasources"][0]["activeCount"], 0);
    let missing = ds["datasources"][0]["unavailableFields"]
        .as_array()
        .unwrap();
    assert!(!missing.contains(&json!("activeCount")));
    assert!(missing.contains(&json!("errorCount")));
    let slow: Value = reqwest::get(format!(
        "{base}/sql?minMaxTimespan=1000&search=orders&datasourceId=1"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(slow["total"], 1);
    assert_eq!(slow["rows"][0]["id"], 7);
    let detail: Value = reqwest::get(format!("{base}/sql/7?datasourceId=1"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(detail["sql"]["parsedTable"]
        .as_array()
        .unwrap()
        .contains(&json!("orders")));
    let stack: Value = reqwest::get(format!("{base}/datasources/1/stack"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stack["supported"], true);
    assert_eq!(stack["rows"], json!(["checkout -> query"]));
    let overflow: Value = reqwest::get(format!(
        "{base}/sql?page=18446744073709551615&perPageCount=500"
    ))
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    assert_eq!(overflow["rows"], json!([]));
    task.abort();
    node_task.abort();
}

#[tokio::test]
async fn failed_poll_is_not_reported_as_an_empty_healthy_source() {
    use druid_admin::config::{AdminDiscoveryConfig, AdminDiscoveryMode};
    for (repo, expected) in [(MetricsRepository::new(), 502), (repository(), 200)] {
        let config = AdminDiscoveryConfig {
            mode: AdminDiscoveryMode::Both,
            applications: vec!["unreachable".into()],
            endpoints: vec!["http://127.0.0.1:1".into()],
        };
        let source = config.build_management_source(repo).unwrap();
        let app = druid_admin::MonitorViewServlet::from_management_source(source).router();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let response = reqwest::get(format!("http://{addr}/druid/api/v1/datasources"))
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
        let body: Value = response.json().await.unwrap();
        assert_eq!(body["partial"], true);
        assert_eq!(body["failures"].as_array().unwrap().len(), 1);
        if expected == 200 {
            assert_eq!(body["datasources"].as_array().unwrap().len(), 2);
        }
        task.abort();
    }
}
