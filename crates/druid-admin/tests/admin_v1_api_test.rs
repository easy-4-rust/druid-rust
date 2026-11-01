//! Tests for the versioned management API the separated console reads.
//!
//! The contract is: one naming convention, one status-code vocabulary and
//! one content type. The Java-compatible routes keep their mixed envelope;
//! the versioned routes must not inherit it.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use serde_json::Value;

use druid_admin::repository::MetricsRepository;

async fn start_admin(
    repository: MetricsRepository,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let servlet = druid_admin::MonitorViewServlet::from_repository(repository);
    let app = servlet.router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

async fn get_json(addr: SocketAddr, path: &str) -> (u16, String, Value) {
    let response = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body = response.text().await.unwrap();
    let json = serde_json::from_str(&body).unwrap_or(Value::Null);
    (status, content_type, json)
}

#[tokio::test]
async fn v1_basic_reports_the_api_version_and_the_backend() {
    let (addr, _handle) = start_admin(MetricsRepository::new()).await;

    let (status, content_type, body) = get_json(addr, "/druid/api/v1/basic").await;

    assert_eq!(status, 200);
    assert!(
        content_type.starts_with("application/json"),
        "the versioned API must answer JSON, got {content_type}"
    );
    assert_eq!(body["apiVersion"], "v1");
    assert_eq!(body["backend"], "push");
    assert!(body["version"].is_string());
    assert!(body["drivers"].is_array());
}

#[tokio::test]
async fn v1_datasources_use_camel_case_and_no_java_envelope() {
    let repository = MetricsRepository::new();
    let (datasource_id, entry) = common::make_datasource_entry(7, "sqlite", 1);
    repository.upsert_datasource(datasource_id, entry);
    let (addr, _handle) = start_admin(repository).await;

    let (status, _content_type, body) = get_json(addr, "/druid/api/v1/datasources").await;

    assert_eq!(status, 200);
    assert!(
        body.get("ResultCode").is_none(),
        "the versioned API must not use the Java envelope, got {body}"
    );
    let rows = body["datasources"].as_array().expect("datasources must be a list");
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row["identity"], 7);
    assert_eq!(row["dbType"], "sqlite");
    assert!(
        row.get("ActiveCount").is_none(),
        "the versioned API must not use Java PascalCase fields, got {row}"
    );
    assert!(row["activeCount"].is_number());
}

#[tokio::test]
async fn v1_datasource_unknown_identity_answers_not_found() {
    let (addr, _handle) = start_admin(MetricsRepository::new()).await;

    let (status, _content_type, body) = get_json(addr, "/druid/api/v1/datasources/999").await;

    assert_eq!(
        status, 404,
        "an unknown identity must answer not-found, got {body}"
    );
}

#[tokio::test]
async fn v1_datasource_returns_the_matching_datasource() {
    let repository = MetricsRepository::new();
    let (datasource_id, entry) = common::make_datasource_entry(7, "sqlite", 1);
    repository.upsert_datasource(datasource_id, entry);
    let (addr, _handle) = start_admin(repository).await;

    let (status, _content_type, body) = get_json(addr, "/druid/api/v1/datasources/7").await;

    assert_eq!(status, 200);
    assert_eq!(body["datasource"]["identity"], 7);
}

#[tokio::test]
async fn v1_sql_sorts_and_pages() {
    let repository = MetricsRepository::new();
    let (datasource_id, mut entry) =
        common::make_datasource_entry_with_sql(3, "sqlite", 1, 11, "select 1", 5);
    let slow = entry.sql_stats.get_mut(&11).expect("SQL stat must exist");
    slow.id = 11;
    let mut faster = slow.clone();
    faster.id = 12;
    faster.sql = Some("select 2".to_owned());
    faster.execute_count = 50;
    entry.sql_stats.insert(12, faster);
    repository.upsert_datasource(datasource_id, entry);
    let (addr, _handle) = start_admin(repository).await;

    let (status, _content_type, body) =
        get_json(addr, "/druid/api/v1/sql?orderBy=executeCount&orderType=desc").await;

    assert_eq!(status, 200);
    assert_eq!(body["total"], 2);
    let rows = body["rows"].as_array().expect("rows must be a list");
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0]["executeCount"], 50,
        "descending order must put the highest count first, got {rows:?}"
    );
}

#[tokio::test]
async fn v1_sql_pages_with_the_requested_window() {
    let repository = MetricsRepository::new();
    let (datasource_id, mut entry) =
        common::make_datasource_entry_with_sql(3, "sqlite", 1, 11, "select 1", 5);
    let mut second = entry.sql_stats.get(&11).expect("SQL stat must exist").clone();
    second.id = 12;
    second.sql = Some("select 2".to_owned());
    entry.sql_stats.insert(12, second);
    repository.upsert_datasource(datasource_id, entry);
    let (addr, _handle) = start_admin(repository).await;

    let (status, _content_type, body) =
        get_json(addr, "/druid/api/v1/sql?page=2&perPageCount=1").await;

    assert_eq!(status, 200);
    assert_eq!(body["total"], 2);
    assert_eq!(body["page"], 2);
    let rows = body["rows"].as_array().expect("rows must be a list");
    assert_eq!(rows.len(), 1, "the second page must hold one row");
}

#[tokio::test]
async fn v1_wall_reports_the_aggregated_counters() {
    let repository = MetricsRepository::new();
    let (datasource_id, entry) = common::make_datasource_entry_with_wall(2, "sqlite", 1, 17);
    repository.upsert_datasource(datasource_id, entry);
    let (addr, _handle) = start_admin(repository).await;

    let (status, _content_type, body) = get_json(addr, "/druid/api/v1/wall").await;

    assert_eq!(status, 200);
    assert_eq!(body["wall"]["checkCount"], 17);
}

#[tokio::test]
async fn v1_services_lists_the_attached_services() {
    let repository = MetricsRepository::new();
    let (datasource_id, entry) = common::make_datasource_entry(7, "sqlite", 1);
    repository.upsert_datasource(datasource_id, entry);
    let (addr, _handle) = start_admin(repository).await;

    let (status, _content_type, body) = get_json(addr, "/druid/api/v1/services").await;

    assert_eq!(status, 200);
    assert!(body["services"].is_array());
}

#[tokio::test]
async fn v1_openapi_document_lists_every_served_route() {
    let (addr, _handle) = start_admin(MetricsRepository::new()).await;

    let (status, _content_type, body) = get_json(addr, "/druid/api/v1/openapi.json").await;

    assert_eq!(status, 200);
    assert_eq!(body["openapi"], "3.1.0");
    let paths = body["paths"].as_object().expect("paths must be an object");
    for route in [
        "/druid/api/v1/basic",
        "/druid/api/v1/datasources",
        "/druid/api/v1/datasources/{identity}",
        "/druid/api/v1/sql",
        "/druid/api/v1/wall",
        "/druid/api/v1/services",
    ] {
        assert!(
            paths.contains_key(route),
            "the document must describe {route}, got {:?}",
            paths.keys().collect::<Vec<_>>()
        );
    }
}
