#![allow(clippy::unused_async)]
use axum::extract::{OriginalUri, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::any;
use axum::Router;
use std::sync::Arc;

/// 将宿主提供的统计服务暴露为 Axum 管理端点。
///
/// 对应 Java：`com.alibaba.druid.support.http.StatViewServlet` 的 JSON 服务部分。
/// UI 静态资源与登录由 `MonitorViewServlet` 承担；本对象用于被监控应用挂载
/// `/druid/*.json`，供独立 `druid-admin` 聚合。
#[derive(Clone)]
pub struct StatViewServlet {
    context_path: String,
    service: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

impl StatViewServlet {
    /// 创建管理 servlet；参数 `service` 接收原 Java servlet 风格 URL 并返回 JSON。
    /// 空 `context_path` 会规范为 `/druid`；返回可挂载的路由对象。
    #[must_use]
    pub fn new(
        context_path: impl Into<String>,
        service: impl Fn(&str) -> String + Send + Sync + 'static,
    ) -> Self {
        let context_path = normalize_context_path(context_path.into());
        Self {
            context_path,
            service: Arc::new(service),
        }
    }

    /// 返回可独立 merge/nest 的 Axum Router。
    pub fn router(self) -> Router {
        let context_path = self.context_path.clone();
        let routes = Router::new()
            .route("/", any(Self::dispatch))
            .route("/{*path}", any(Self::dispatch))
            .with_state(self);
        Router::new().nest(&context_path, routes)
    }

    async fn dispatch(State(state): State<Self>, OriginalUri(uri): OriginalUri) -> Response {
        let full = uri
            .path_and_query()
            .map_or_else(|| uri.path().to_owned(), ToString::to_string);
        let service_url = full
            .strip_prefix(&state.context_path)
            .filter(|value| !value.is_empty())
            .unwrap_or("/");
        let body = (state.service)(service_url);
        let mut response = Response::new(body.into());
        *response.status_mut() = StatusCode::OK;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json;charset=utf-8"),
        );
        response
    }
}

impl std::fmt::Debug for StatViewServlet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StatViewServlet")
            .field("context_path", &self.context_path)
            .field("service", &"[INJECTED]")
            .finish()
    }
}

fn normalize_context_path(mut context_path: String) -> String {
    if context_path.is_empty() || context_path == "/" {
        return "/druid".to_owned();
    }
    if !context_path.starts_with('/') {
        context_path.insert(0, '/');
    }
    while context_path.ends_with('/') {
        context_path.pop();
    }
    context_path
}
