//! RED tests for Java-compatible REST/JSON API.
//!
//! Task 2 Step 1: Tests for /druid/datasource.json, /druid/sql.json,
//! /druid/wall.json, and /druid/api.json routes.

use std::time::Duration;

use serde_json::Value;

/// Helper: start the standalone HTTP server and return (addr, handle).
async fn start_server() -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let repo = druid_admin::repository::MetricsRepository::new();
    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

/// /druid/datasource.json must return a Java-compatible response with
/// `ResultCode` and Content fields.
#[tokio::test]
async fn datasource_json_returns_java_format() {
    let (addr, handle) = start_server().await;

    let resp = reqwest::get(format!("http://{addr}/druid/datasource.json"))
        .await
        .expect("GET /druid/datasource.json must succeed");

    // Even with empty repo, must return 200 with correct structure
    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("must be JSON");
    assert!(
        body.get("ResultCode").is_some(),
        "response must contain ResultCode"
    );
    assert!(
        body.get("Content").is_some(),
        "response must contain Content"
    );

    handle.abort();
}

/// /druid/sql.json must return a Java-compatible response.
#[tokio::test]
async fn sql_json_returns_java_format() {
    let (addr, handle) = start_server().await;

    let resp = reqwest::get(format!("http://{addr}/druid/sql.json"))
        .await
        .expect("GET /druid/sql.json must succeed");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("must be JSON");
    assert!(
        body.get("ResultCode").is_some(),
        "response must contain ResultCode"
    );

    handle.abort();
}

/// /druid/wall.json must return a Java-compatible response.
#[tokio::test]
async fn wall_json_returns_java_format() {
    let (addr, handle) = start_server().await;

    let resp = reqwest::get(format!("http://{addr}/druid/wall.json"))
        .await
        .expect("GET /druid/wall.json must succeed");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("must be JSON");
    assert!(
        body.get("ResultCode").is_some(),
        "response must contain ResultCode"
    );

    handle.abort();
}

/// /druid/api.json must return an API description with endpoint list.
#[tokio::test]
async fn api_json_returns_endpoint_list() {
    let (addr, handle) = start_server().await;

    let resp = reqwest::get(format!("http://{addr}/druid/api.json"))
        .await
        .expect("GET /druid/api.json must succeed");

    assert_eq!(resp.status(), 200);

    let body: Value = resp.json().await.expect("must be JSON");
    assert!(
        body.get("ResultCode").is_some(),
        "response must contain ResultCode"
    );
    assert!(
        body.get("Content").is_some(),
        "response must contain Content"
    );

    handle.abort();
}

/// The datasource.json endpoint must return Content-Type: application/json.
#[tokio::test]
async fn datasource_json_content_type() {
    let (addr, handle) = start_server().await;

    let resp = reqwest::get(format!("http://{addr}/druid/datasource.json"))
        .await
        .expect("GET must succeed");

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        content_type.contains("application/json"),
        "Content-Type must be application/json, got: {content_type}"
    );

    handle.abort();
}

/// /druid/datasource.json with data in repo must return populated Content.
#[tokio::test]
async fn datasource_json_returns_populated_content() {
    use druid_admin::model::dto::DataSourceContent;
    use druid_admin::repository::{DataSourceEntry, DataSourceId};
    use druid_metrics::ServiceIdentity;

    let repo = druid_admin::repository::MetricsRepository::new();

    // Insert a test datasource entry
    let id = DataSourceId {
        service_identity: ServiceIdentity::new("test-service", "test-instance", "test-boot"),
        identity: 1,
    };
    let entry = DataSourceEntry {
        datasource: DataSourceContent {
            identity: 1,
            db_type: Some("mysql".to_owned()),
            url: Some("jdbc:mysql://localhost:3306/test".to_owned()),
            ..Default::default()
        },
        sql_stats: std::collections::HashMap::new(),
        wall: Default::default(),
        last_updated_ms: 0,
        sequence: 1,
    };
    repo.upsert_datasource(id, entry);

    let app = druid_admin::standalone_router(repo);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    let resp = reqwest::get(format!("http://{addr}/druid/datasource.json"))
        .await
        .expect("GET must succeed");

    let body: Value = resp.json().await.expect("must be JSON");
    let content = body
        .get("Content")
        .and_then(|c| c.as_array())
        .expect("Content must be an array");
    assert_eq!(content.len(), 1, "must have one datasource entry");

    let ds = &content[0];
    assert_eq!(
        ds.get("DbType").and_then(|v| v.as_str()),
        Some("mysql"),
        "DbType must be mysql"
    );

    handle.abort();
}

#[tokio::test]
async fn detail_routes_use_service_datasource_and_sql_composite_identity() {
    use std::collections::HashMap;

    use druid_admin::model::dto::{DataSourceContent, SqlListContent, WallResult};
    use druid_admin::repository::{DataSourceEntry, DataSourceId, MetricsRepository};
    use druid_metrics::ServiceIdentity;

    let repository = MetricsRepository::new();
    for (service_name, datasource_id, db_type, sql) in [
        ("svc-a", 1, "mysql", "svc-a-ds-1"),
        ("svc-a", 2, "postgresql", "svc-a-ds-2"),
        ("svc-b", 1, "sqlite", "svc-b-ds-1"),
    ] {
        let service_identity = ServiceIdentity::new(service_name, "detail-instance", "detail-boot");
        let service_id = service_identity.service_id();
        let mut sql_stats = HashMap::new();
        sql_stats.insert(
            101,
            SqlListContent {
                service_id: Some(service_id.clone()),
                service_name: Some(service_name.to_owned()),
                instance_id: Some("detail-instance".to_owned()),
                boot_id: Some("detail-boot".to_owned()),
                id: 1,
                hash: 101,
                sql: Some(sql.to_owned()),
                data_source: serde_json::json!(datasource_id),
                ..Default::default()
            },
        );
        assert!(repository.upsert_datasource(
            DataSourceId {
                service_identity,
                identity: datasource_id,
            },
            DataSourceEntry {
                datasource: DataSourceContent {
                    service_id: Some(service_id.clone()),
                    service_name: Some(service_name.to_owned()),
                    instance_id: Some("detail-instance".to_owned()),
                    boot_id: Some("detail-boot".to_owned()),
                    identity: datasource_id,
                    db_type: Some(db_type.to_owned()),
                    ..Default::default()
                },
                sql_stats,
                wall: WallResult::default(),
                last_updated_ms: 0,
                sequence: 1,
            },
        ));
    }
    let app = druid_admin::standalone_router(repository);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind composite detail HTTP");
    let address = listener.local_addr().expect("composite detail address");
    let handle = tokio::spawn(async move { axum::serve(listener, app).await });

    let service_a = ServiceIdentity::new("svc-a", "detail-instance", "detail-boot").service_id();
    let service_b = ServiceIdentity::new("svc-b", "detail-instance", "detail-boot").service_id();

    let datasource: Value = reqwest::get(format!(
        "http://{address}/druid/api/datasources/1?serviceId={service_b}"
    ))
    .await
    .expect("GET composite datasource")
    .error_for_status()
    .expect("composite datasource found")
    .json()
    .await
    .expect("composite datasource JSON");
    assert_eq!(datasource["Content"][0]["serviceId"], service_b);
    assert_eq!(datasource["Content"][0]["DbType"], "sqlite");

    let sql: Value = reqwest::get(format!(
        "http://{address}/druid/api/sql/1?serviceId={service_a}&dataSourceId=2"
    ))
    .await
    .expect("GET composite SQL")
    .error_for_status()
    .expect("composite SQL found")
    .json()
    .await
    .expect("composite SQL JSON");
    assert_eq!(sql["serviceId"], service_a);
    assert_eq!(sql["DataSource"], 2);
    assert_eq!(sql["SQL"], "svc-a-ds-2");

    let missing_datasource = reqwest::get(format!(
        "http://{address}/druid/api/sql/1?serviceId={service_a}"
    ))
    .await
    .expect("GET SQL without datasource identity");
    assert_eq!(
        missing_datasource.status(),
        reqwest::StatusCode::UNPROCESSABLE_ENTITY
    );
    handle.abort();
}
