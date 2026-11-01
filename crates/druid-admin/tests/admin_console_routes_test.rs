//! Tests for the Java-compatible management routes the console pages call.
//!
//! The bundled console pages request `basic.json`, `datasource-{id}.json`,
//! `sql-{id}.json`, `connectionInfo-{id}.json`,
//! `activeConnectionStackTrace-{id}.json`, `reset-all.json` and
//! `log-and-reset.json`. Every one of those routes must answer, either with
//! the Java-compatible envelope or with an explicit "not supported by this
//! backend" reason — never with a generic failure that leaves the page blank.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use serde_json::Value;

use druid_admin::command::CommandBroker;
use druid_admin::repository::MetricsRepository;

/// Build an Admin HTTP server over the push repository backend.
async fn start_admin_with_repository(
    repository: MetricsRepository,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let broker = CommandBroker::new(16, Duration::from_secs(5));
    let servlet = druid_admin::MonitorViewServlet::from_repository(repository)
        .with_command_broker(broker);
    let app = servlet.router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

async fn get_json(addr: SocketAddr, path: &str) -> (u16, Value) {
    let response = reqwest::get(format!("http://{addr}{path}")).await.unwrap();
    let status = response.status().as_u16();
    let body = response.text().await.unwrap();
    let json = serde_json::from_str(&body).unwrap_or(Value::Null);
    (status, json)
}

#[tokio::test]
async fn basic_json_reports_the_runtime_version_and_reset_state() {
    let (addr, _handle) = start_admin_with_repository(MetricsRepository::new()).await;

    let (status, body) = get_json(addr, "/druid/basic.json").await;

    assert_eq!(status, 200, "basic.json must be served");
    assert_eq!(body["ResultCode"], 1, "basic.json must report success");
    let content = &body["Content"];
    assert!(
        content["Version"].is_string(),
        "basic.json must report a runtime version, got {content}"
    );
    assert!(
        content["Drivers"].is_array(),
        "basic.json must report the known drivers, got {content}"
    );
    assert_eq!(
        content["ResetEnable"], true,
        "basic.json must report the reset flag"
    );
    assert!(
        content["ResetCount"].is_number(),
        "basic.json must report the reset count, got {content}"
    );
    assert!(
        content["StartTime"].is_string() || content["StartTime"].is_number(),
        "basic.json must report the start time, got {content}"
    );
}

#[tokio::test]
async fn datasource_identity_json_returns_the_matching_datasource() {
    let repository = MetricsRepository::new();
    let (datasource_id, entry) = common::make_datasource_entry(7, "sqlite", 1);
    repository.upsert_datasource(datasource_id, entry);
    let (addr, _handle) = start_admin_with_repository(repository).await;

    let (status, body) = get_json(addr, "/druid/datasource-7.json").await;

    assert_eq!(status, 200, "datasource-{{id}}.json must be served");
    assert_eq!(body["ResultCode"], 1);
    let content = &body["Content"];
    let rows = content.as_array().expect("Content must be a list");
    assert_eq!(rows.len(), 1, "exactly one datasource must be returned");
    assert_eq!(rows[0]["Identity"], 7);
    assert_eq!(rows[0]["DbType"], "sqlite");
}

#[tokio::test]
async fn datasource_identity_json_unknown_identity_reports_not_found() {
    let (addr, _handle) = start_admin_with_repository(MetricsRepository::new()).await;

    let (status, body) = get_json(addr, "/druid/datasource-999.json").await;

    assert_eq!(status, 200, "the compatibility route keeps the Java envelope");
    assert_eq!(
        body["ResultCode"], -1,
        "an unknown identity must report failure, got {body}"
    );
    assert!(
        body["Content"].as_str().is_some_and(|value| !value.is_empty()),
        "an unknown identity must carry a reason, got {body}"
    );
}

#[tokio::test]
async fn sql_identity_json_returns_the_matching_sql_stat() {
    let repository = MetricsRepository::new();
    let (datasource_id, mut entry) =
        common::make_datasource_entry_with_sql(3, "sqlite", 1, 4242, "select 1", 9);
    // SQL 身份同时可以按统计 ID 与按指纹寻址，这里用统计 ID。
    let stat = entry.sql_stats.get_mut(&4242).expect("SQL stat must exist");
    stat.id = 4242;
    repository.upsert_datasource(datasource_id, entry);
    let (addr, _handle) = start_admin_with_repository(repository).await;

    let (status, body) = get_json(addr, "/druid/sql-4242.json").await;

    assert_eq!(status, 200, "sql-{{id}}.json must be served");
    assert_eq!(body["ResultCode"], 1);
    let content = &body["Content"];
    assert_eq!(content["SQL"], "select 1");
    assert_eq!(content["ExecuteCount"], 9);
}

#[tokio::test]
async fn sql_identity_json_unknown_identity_reports_not_found() {
    let (addr, _handle) = start_admin_with_repository(MetricsRepository::new()).await;

    let (status, body) = get_json(addr, "/druid/sql-12345.json").await;

    assert_eq!(status, 200);
    assert_eq!(
        body["ResultCode"], -1,
        "an unknown SQL identity must report failure, got {body}"
    );
}

#[tokio::test]
async fn connection_info_identity_json_reports_the_capability_the_backend_lacks() {
    let (addr, _handle) = start_admin_with_repository(MetricsRepository::new()).await;

    let (status, body) = get_json(addr, "/druid/connectionInfo-1.json").await;

    assert_eq!(status, 200, "the compatibility route keeps the Java envelope");
    assert_eq!(
        body["ResultCode"], -1,
        "the push backend cannot answer connection info, got {body}"
    );
    let reason = body["Content"].as_str().unwrap_or_default();
    assert!(
        reason.contains("connection"),
        "the reason must name the missing capability, got {reason}"
    );
}

#[tokio::test]
async fn active_connection_stack_trace_identity_json_reports_the_capability_the_backend_lacks()
{
    let (addr, _handle) = start_admin_with_repository(MetricsRepository::new()).await;

    let (status, body) = get_json(addr, "/druid/activeConnectionStackTrace-1.json").await;

    assert_eq!(status, 200);
    assert_eq!(
        body["ResultCode"], -1,
        "the push backend cannot answer stack traces, got {body}"
    );
    let reason = body["Content"].as_str().unwrap_or_default();
    assert!(
        reason.contains("stack"),
        "the reason must name the missing capability, got {reason}"
    );
}

#[tokio::test]
async fn reset_all_json_is_accepted_and_reports_success() {
    let (addr, _handle) = start_admin_with_repository(MetricsRepository::new()).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/druid/reset-all.json"))
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap_or(Value::Null);

    assert_eq!(status, 200, "reset-all.json must be served");
    assert_eq!(
        body["ResultCode"], 1,
        "reset-all.json must report success when no service is attached, got {body}"
    );
}

#[tokio::test]
async fn log_and_reset_json_is_accepted_and_reports_success() {
    let (addr, _handle) = start_admin_with_repository(MetricsRepository::new()).await;

    let response = reqwest::Client::new()
        .post(format!("http://{addr}/druid/log-and-reset.json"))
        .send()
        .await
        .unwrap();
    let status = response.status().as_u16();
    let body: Value = serde_json::from_str(&response.text().await.unwrap()).unwrap_or(Value::Null);

    assert_eq!(status, 200, "log-and-reset.json must be served");
    assert_eq!(
        body["ResultCode"], 1,
        "log-and-reset.json must report success when no service is attached, got {body}"
    );
}
