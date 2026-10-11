//! Druid 管理端的 Axum 实现。
//!
//! 对应 Java 模块：`druid-admin`。本 crate 保留 Java 管理协议和 JSON
//! 字段，同时用可注入的发现与 SPI 替代 Spring Cloud/Kubernetes
//! 静态依赖。Standalone binary 与兼容 facade 都委托同一个
//! [`MonitorViewServlet`] 应用装配路径。

pub mod admin_state;
pub mod command;
pub mod config;
pub mod druid_admin_application;
pub mod ingest;
pub mod model;
pub mod repository;
pub mod security;
pub mod service;
pub mod servlet;
pub mod util;

pub use admin_state::AdminState;
pub use command::CommandBroker;
pub use druid_admin_application::DruidAdminApplication;
pub use repository::MetricsRepository;
pub use servlet::monitor_view_servlet::{endpoint_list, MonitorViewServlet};
pub use servlet::StatViewServlet;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use tower_http::trace::TraceLayer;

use crate::config::AdminServerConfig;

/// Standalone Admin 的最大 HTTP request body（1 MiB）。
const MAX_BODY_SIZE: usize = 1 << 20;

/// 构建生产 HTTP 应用：health public，其余管理端点由同一个 Servlet 认证边界保护。
#[must_use]
pub fn production_router(repository: MetricsRepository, config: &AdminServerConfig) -> Router {
    production_router_with_command_broker(
        repository,
        config,
        CommandBroker::new(64, config.shutdown_timeout()),
    )
}

/// 构建与真实 IngestService 共享命令 broker 的生产 HTTP 应用。
#[must_use]
pub fn production_router_with_command_broker(
    repository: MetricsRepository,
    config: &AdminServerConfig,
    command_broker: CommandBroker,
) -> Router {
    let management = MonitorViewServlet::from_repository(repository)
        .with_auth_mode(config.auth().clone())
        .with_secure_cookies(config.tls().is_some())
        .with_command_broker(command_broker)
        .router();
    Router::new()
        .route("/health", get(health_handler))
        .merge(management)
        .layer(axum::extract::DefaultBodyLimit::max(MAX_BODY_SIZE))
        .layer(TraceLayer::new_for_http().make_span_with(
            |request: &axum::http::Request<axum::body::Body>| {
                tracing::debug_span!(
                    "request",
                    method = %request.method(),
                    path = request.uri().path(),
                    version = ?request.version(),
                )
            },
        ))
}

/// 构建 loopback 兼容 facade；路由与 repository 投影仍委托生产装配路径。
#[must_use]
pub fn standalone_router(repository: MetricsRepository) -> Router {
    production_router(repository, &AdminServerConfig::default())
}

async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok"})))
}
