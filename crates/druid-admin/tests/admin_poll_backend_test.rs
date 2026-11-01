//! Tests for the poll backend: the Admin fans out to the monitored node's
//! `/druid/*.json` and serves the Java-equivalent deep data that the push
//! protocol cannot carry.

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

use druid_admin::config::{AdminDiscoveryConfig, AdminDiscoveryMode};
use druid_admin::repository::MetricsRepository;

/// 启动一个假的被监控节点，提供 Java 兼容的 `/druid/*.json`。
async fn start_fake_node() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/druid/datasource.json",
            get(|| async {
                Json(json!({
                    "ResultCode": 1,
                    "Content": [{
                        "Identity": 1,
                        "Name": "order-datasource",
                        "DbType": "postgresql",
                        "DriverClassName": "postgres",
                        "URL": "jdbc:postgresql://10.0.0.7:5432/orders",
                        "UserName": "orders_app",
                        "ActiveCount": 12,
                        "PoolingCount": 8,
                        "MaxActive": 64,
                        "ExecuteCount": 9482,
                        "CommitCount": 4410,
                        "RollbackCount": 3,
                    }],
                }))
            }),
        )
        .route(
            "/druid/sql.json",
            get(|| async {
                Json(json!({
                    "ResultCode": 1,
                    "Content": [{
                        "ID": 7,
                        "SQL": "select * from orders where id = ?",
                        "ExecuteCount": 4820,
                        "TotalTime": 37,
                        "DbType": "postgresql",
                    }],
                }))
            }),
        )
        // 活跃连接堆栈：id=1 返回两条借出堆栈，其余 id 报 removeAbandoned 未开启。
        .route(
            "/druid/activeConnectionStackTrace-1.json",
            get(|| async {
                Json(json!({
                    "ResultCode": 1,
                    "Content": [
                        "   0: druid::pool::borrow\n   1: app::checkout",
                        "   0: druid::pool::borrow\n   1: app::report_job",
                    ],
                }))
            }),
        )
        .route(
            "/druid/activeConnectionStackTrace-99.json",
            get(|| async {
                Json(json!({
                    "ResultCode": -1,
                    "Content": "require set removeAbandoned=true",
                }))
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (addr, handle)
}

/// 用给定来源启动 Admin。
async fn start_admin_with_source(
    source: druid_admin::service::ManagementSource,
) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let servlet = druid_admin::MonitorViewServlet::from_management_source(source);
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
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn poll_mode_serves_the_monitored_node_rich_data() {
    let (node_addr, _node) = start_fake_node().await;

    let discovery = AdminDiscoveryConfig {
        mode: AdminDiscoveryMode::Poll,
        applications: vec!["orders".to_owned()],
        endpoints: vec![format!("http://{node_addr}")],
    };
    let source = discovery
        .build_management_source(MetricsRepository::new())
        .expect("source must build");
    let (addr, _handle) = start_admin_with_source(source).await;

    // Java 兼容路由拿到的是远端节点的深层数据。
    let (status, body) = get_json(addr, "/druid/datasource.json").await;
    assert_eq!(status, 200);
    // Java Admin 的 datasource.json 返回 ResultCode 0（DTO 默认值），与实现平价。
    let rows = body["Content"]
        .as_array()
        .expect("Content must be a list, got {body}");
    assert_eq!(rows.len(), 1, "the monitored node must be polled");
    assert_eq!(rows[0]["URL"], "jdbc:postgresql://10.0.0.7:5432/orders");
    assert_eq!(rows[0]["UserName"], "orders_app");

    // 版本化 API 同样拿到这些深层字段。
    let (status, body) = get_json(addr, "/druid/api/v1/datasources").await;
    assert_eq!(status, 200);
    let rows = body["datasources"]
        .as_array()
        .expect("datasources must be a list");
    assert_eq!(rows[0]["url"], "jdbc:postgresql://10.0.0.7:5432/orders");
    assert_eq!(rows[0]["userName"], "orders_app");
}

#[tokio::test]
async fn both_mode_falls_back_to_the_repository_when_the_node_is_unreachable() {
    // 指向一个不存在的节点：远端无数据，回落到推送仓库。
    let discovery = AdminDiscoveryConfig {
        mode: AdminDiscoveryMode::Both,
        applications: vec!["orders".to_owned()],
        endpoints: vec!["http://127.0.0.1:1".to_owned()],
    };
    let repository = MetricsRepository::new();
    let (datasource_id, entry) = common::make_datasource_entry(9, "sqlite", 1);
    repository.upsert_datasource(datasource_id, entry);
    let source = discovery
        .build_management_source(repository)
        .expect("source must build");
    let (addr, _handle) = start_admin_with_source(source).await;

    let (status, body) = get_json(addr, "/druid/api/v1/datasources").await;
    assert_eq!(status, 200);
    let rows = body["datasources"]
        .as_array()
        .expect("datasources must be a list");
    assert_eq!(
        rows.len(),
        1,
        "the repository fallback must serve the pushed datasource"
    );
    assert_eq!(rows[0]["identity"], 9);
    assert_eq!(rows[0]["dbType"], "sqlite");
}

#[tokio::test]
async fn service_list_reports_the_configured_application() {
    let (node_addr, _node) = start_fake_node().await;
    let discovery = AdminDiscoveryConfig {
        mode: AdminDiscoveryMode::Poll,
        applications: vec!["orders".to_owned()],
        endpoints: vec![format!("http://{node_addr}")],
    };
    let source = discovery
        .build_management_source(MetricsRepository::new())
        .expect("source must build");
    let (addr, _handle) = start_admin_with_source(source).await;

    let (status, body) = get_json(addr, "/druid/serviceList.json").await;
    assert_eq!(status, 200);
    assert!(
        body.as_array()
            .is_some_and(|names| names.iter().any(|n| n == "orders")),
        "serviceList.json must report the configured application, got {body}"
    );
}

#[tokio::test]
async fn poll_mode_serves_active_connection_stack_traces() {
    let (node_addr, _node) = start_fake_node().await;
    let discovery = AdminDiscoveryConfig {
        mode: AdminDiscoveryMode::Poll,
        applications: vec!["orders".to_owned()],
        endpoints: vec![format!("http://{node_addr}")],
    };
    let source = discovery
        .build_management_source(MetricsRepository::new())
        .expect("source must build");
    let (addr, _handle) = start_admin_with_source(source).await;

    let (status, body) = get_json(
        addr,
        "/druid/activeConnectionStackTrace-1.json?serviceId=orders-0",
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(
        body["ResultCode"], 1,
        "the monitored node stack traces must be served, got {body}"
    );
    let traces = body["Content"]
        .as_array()
        .expect("Content must be the stack trace list, got {body}");
    assert_eq!(traces.len(), 2, "the fake node holds two active borrows");
    assert!(
        traces[0]
            .as_str()
            .is_some_and(|trace| trace.contains("borrow")),
        "each entry is one connect-time stack trace, got {traces:?}"
    );
}

#[tokio::test]
async fn poll_mode_forwards_the_node_remove_abandoned_error() {
    let (node_addr, _node) = start_fake_node().await;
    let discovery = AdminDiscoveryConfig {
        mode: AdminDiscoveryMode::Poll,
        applications: vec!["orders".to_owned()],
        endpoints: vec![format!("http://{node_addr}")],
    };
    let source = discovery
        .build_management_source(MetricsRepository::new())
        .expect("source must build");
    let (addr, _handle) = start_admin_with_source(source).await;

    let (status, body) = get_json(
        addr,
        "/druid/activeConnectionStackTrace-99.json?serviceId=orders-0",
    )
    .await;

    assert_eq!(status, 200);
    assert_eq!(
        body["ResultCode"], -1,
        "the node's own failure must be forwarded, got {body}"
    );
    let reason = body["Content"].as_str().unwrap_or_default();
    assert!(
        reason.contains("removeAbandoned"),
        "the reason must be the node's, not a generic admin error, got {reason}"
    );
}
