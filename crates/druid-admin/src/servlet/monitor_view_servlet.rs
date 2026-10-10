#![allow(clippy::case_sensitive_file_extension_comparisons)]
#![allow(clippy::unused_async)] // axum handlers require async signature even when no .await

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Form, OriginalUri, Path, Query, Request, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_valid::Valid;
use druid_metrics::ServiceIdentity;
use parking_lot::RwLock;
use serde_json::{json, Value};
use tokio_metrics::TaskMonitor;

use crate::config::AdminAuthMode;
use crate::model::dto::DataSourceResult;
use crate::repository::{DataSourceId, MetricsRepository};
use crate::service::{MonitorStatService, StatQuery};
use crate::CommandBroker;

use super::LoginRequest;

/// Session expiry duration (2 hours, matching typical Java Servlet session).
const SESSION_TTL: Duration = Duration::from_secs(7200);
const MAX_SESSIONS: usize = 1_024;

/// Druid 管理协议的 Axum 路由。
///
/// 对应 Java: `com.alibaba.druid.admin.servlet.MonitorViewServlet`。
///
/// Sessions are stored with creation timestamps, a two-hour TTL, and a hard
/// capacity so repeated successful logins cannot grow memory without bound.
#[derive(Clone)]
pub struct MonitorViewServlet {
    backend: MonitorBackend,
    task_monitor: TaskMonitor,
    auth: AdminAuthMode,
    /// Session token -> creation instant. Expired entries are pruned on access.
    sessions: Arc<RwLock<HashMap<String, Instant>>>,
    context_path: Arc<String>,
    secure_cookies: bool,
    command_broker: Option<CommandBroker>,
}

#[derive(Clone)]
enum MonitorBackend {
    Remote(Arc<MonitorStatService>),
    Repository(MetricsRepository),
}

/// 返回稳定端点清单；保留早期 `druid-admin` 门面 API。
#[must_use]
pub fn endpoint_list() -> &'static str {
    MonitorViewServlet::endpoint_list()
}

impl MonitorViewServlet {
    /// 创建管理路由对象。
    #[must_use]
    pub fn new(monitor_stat_service: Arc<MonitorStatService>) -> Self {
        Self {
            backend: MonitorBackend::Remote(monitor_stat_service),
            task_monitor: TaskMonitor::new(),
            auth: AdminAuthMode::Disabled,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            context_path: Arc::new("/druid".to_owned()),
            secure_cookies: false,
            command_broker: None,
        }
    }

    /// 使用 Metrics gRPC ingest 的本地 repository 创建 standalone 管理路由。
    #[must_use]
    pub fn from_repository(repository: MetricsRepository) -> Self {
        Self {
            backend: MonitorBackend::Repository(repository),
            task_monitor: TaskMonitor::new(),
            auth: AdminAuthMode::Disabled,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            context_path: Arc::new("/druid".to_owned()),
            secure_cookies: false,
            command_broker: None,
        }
    }

    /// 配置 Java `loginUsername`/`loginPassword` 登录语义。
    #[must_use]
    pub fn with_credentials(
        mut self,
        login_username: Option<String>,
        login_password: Option<String>,
    ) -> Self {
        self.auth = login_username.map_or(AdminAuthMode::Disabled, |username| {
            AdminAuthMode::basic(username, login_password.unwrap_or_default())
        });
        self
    }

    /// 配置 HTTP 与 gRPC 共用的应用层认证模式。
    #[must_use]
    pub fn with_auth_mode(mut self, auth: AdminAuthMode) -> Self {
        self.auth = auth;
        self
    }

    /// 指定 session cookie 是否仅允许通过 TLS 发送。
    #[must_use]
    pub fn with_secure_cookies(mut self, secure_cookies: bool) -> Self {
        self.secure_cookies = secure_cookies;
        self
    }

    /// 注入与生产 gRPC IngestService 共用的有界命令 broker。
    #[must_use]
    pub fn with_command_broker(mut self, command_broker: CommandBroker) -> Self {
        self.command_broker = Some(command_broker);
        self
    }

    /// 配置 Java Servlet 的 context path；空值保持 `/druid`。
    #[must_use]
    pub fn with_context_path(mut self, context_path: Option<String>) -> Self {
        self.context_path = Arc::new(
            context_path
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| "/druid".to_owned()),
        );
        self
    }

    /// 构建 Java 兼容端点和 Rust REST 别名。
    pub fn router(self) -> Router {
        let auth_state = self.clone();
        let context_path = self.context_path.to_string();
        let admin_routes = Router::new()
            .route("/index.html", get(Self::legacy_page))
            .route("/login.html", get(Self::login_page))
            .route("/nopermit.html", get(Self::no_permit_page))
            .route("/datasource.html", get(Self::datasource_page))
            .route("/sql.html", get(Self::sql_page))
            .route("/sql-detail.html", get(Self::sql_detail_page))
            .route("/wall.html", get(Self::wall_page))
            .route("/api.html", get(Self::api_page))
            .route("/spring.html", get(Self::spring_page))
            .route("/spring-detail.html", get(Self::spring_detail_page))
            .route("/weburi.html", get(Self::weburi_page))
            .route("/weburi-detail.html", get(Self::weburi_detail_page))
            .route("/websession.html", get(Self::websession_page))
            .route("/websession-detail.html", get(Self::websession_detail_page))
            .route("/webapp.html", get(Self::webapp_page))
            .route("/connectionInfo.html", get(Self::connection_info_page))
            .route(
                "/activeConnectionStackTrace.html",
                get(Self::active_connection_stack_trace_page),
            )
            .route("/css/{asset}", get(Self::legacy_style))
            .route("/js/{asset}", get(Self::legacy_script))
            .route("/submitLogin", post(Self::submit_login))
            .route("/serviceList.json", get(Self::service_list))
            .route("/datasource.json", get(Self::data_sources))
            .route("/sql.json", get(Self::sql_list))
            .route("/wall.json", get(Self::wall))
            .route("/api.json", get(Self::api_description))
            .route("/weburi.json", get(Self::web_uri_list))
            .route("/webapp.json", get(Self::web_app_list))
            .route("/websession.json", get(Self::web_session_list))
            .route("/spring.json", get(Self::spring_list))
            .route("/spring-detail.json", get(Self::spring_detail))
            .route("/api/datasources", get(Self::data_sources))
            .route("/api/datasources/{id}", get(Self::data_source))
            .route("/api/sql/top", get(Self::sql_list))
            .route("/api/sql/slow", get(Self::sql_list))
            .route("/api/sql/{id}", get(Self::sql_detail))
            .route("/api/wall", get(Self::wall))
            .route("/api/weburi/{uri}", get(Self::web_uri_detail))
            .route("/api/websessions/{id}", get(Self::web_session_detail))
            .route("/api/connections/{id}", get(Self::connection_info))
            .route("/api/active", get(Self::active))
            .route("/api/commands", post(Self::issue_command))
            .fallback(Self::legacy_dispatch);
        // Merge admin routes into a single router that also handles the
        // bare context-path (e.g. /druid and /druid/) by redirecting to
        // the index page.
        let context_slash = format!("{context_path}/");
        Router::new()
            .route(&context_path, get(Self::root))
            .route(&context_slash, get(Self::root))
            .merge(
                Router::new()
                    .nest(&context_path, admin_routes)
                    .route("/metrics", get(Self::metrics)),
            )
            .with_state(self)
            .layer(middleware::from_fn_with_state(auth_state, Self::authorize))
    }

    /// 返回稳定端点清单。
    #[must_use]
    pub fn endpoint_list() -> &'static str {
        r#"["/druid/api/datasources","/druid/api/sql/top","/druid/api/sql/slow","/druid/api/wall","/druid/api/active","/metrics"]"#
    }

    async fn authorize(State(state): State<Self>, request: Request, next: Next) -> Response {
        if is_command_path(request.uri().path(), state.context_path.as_str()) {
            // 命令端点必须由 handler 校验显式 Basic，不能被 Disabled 或 session 放行。
            return next.run(request).await;
        }
        if !state.auth.is_enabled() {
            return next.run(request).await;
        }
        let authorization = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        if is_public_path(request.uri().path(), state.context_path.as_str())
            || request_has_session(&request, &state.sessions)
            || state.auth.authorizes_header(authorization)
        {
            return next.run(request).await;
        }
        redirect_found(format!("{}/login.html", state.context_path))
    }

    async fn issue_command(
        State(state): State<Self>,
        headers: HeaderMap,
        Json(body): Json<Value>,
    ) -> Response {
        let authorization = headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok());
        if !state.auth.is_enabled() {
            return command_error_response(
                StatusCode::FORBIDDEN,
                "command entry is unavailable without configured Basic credentials",
            );
        }
        if !state.auth.authorizes_header(authorization) {
            return command_error_response(
                StatusCode::UNAUTHORIZED,
                "command entry requires explicit Authorization",
            );
        }
        let Some(broker) = state.command_broker.as_ref() else {
            return command_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "command broker is unavailable",
            );
        };
        let parsed = (|| {
            let service_name = required_json_string(&body, "serviceName")?;
            let instance_id = required_json_string(&body, "instanceId")?;
            let boot_id = required_json_string(&body, "bootId")?;
            let command_id = required_json_string(&body, "commandId")?;
            let command_type = required_json_string(&body, "type")?;
            Ok::<_, String>((service_name, instance_id, boot_id, command_id, command_type))
        })();
        let (service_name, instance_id, boot_id, command_id, command_type) = match parsed {
            Ok(parsed) => parsed,
            Err(error) => return command_error_response(StatusCode::BAD_REQUEST, &error),
        };
        let result = match command_type {
            "resetStats" => {
                let Some(ids) = body.get("targetDatasourceIds").and_then(Value::as_array) else {
                    return command_error_response(
                        StatusCode::BAD_REQUEST,
                        "targetDatasourceIds must be an array",
                    );
                };
                let ids = ids
                    .iter()
                    .map(|value| {
                        value.as_u64().ok_or_else(|| {
                            "targetDatasourceIds must contain unsigned integers".to_owned()
                        })
                    })
                    .collect::<Result<Vec<_>, _>>();
                let ids = match ids {
                    Ok(ids) if !ids.is_empty() => ids,
                    Ok(_) => {
                        return command_error_response(
                            StatusCode::BAD_REQUEST,
                            "targetDatasourceIds must not be empty",
                        );
                    }
                    Err(error) => return command_error_response(StatusCode::BAD_REQUEST, &error),
                };
                broker
                    .issue_reset(service_name, instance_id, boot_id, command_id, ids)
                    .await
            }
            "requestFullSnapshot" => {
                broker
                    .issue_full_snapshot(service_name, instance_id, boot_id, command_id)
                    .await
            }
            _ => {
                return command_error_response(StatusCode::BAD_REQUEST, "unsupported command type");
            }
        };
        match result {
            Ok(acknowledgement) if acknowledgement.success => (
                StatusCode::OK,
                Json(json!({
                    "commandId": acknowledgement.command_id,
                    "success": true
                })),
            )
                .into_response(),
            Ok(acknowledgement) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "commandId": acknowledgement.command_id,
                    "success": false,
                    "error": acknowledgement.error_message
                })),
            )
                .into_response(),
            Err(error) => command_error_response(command_error_status(&error), &error),
        }
    }

    async fn root(State(state): State<Self>) -> Response {
        redirect_found(format!("{}/index.html", state.context_path))
    }

    async fn legacy_page() -> Response {
        static_resource_response("index.html")
    }

    async fn datasource_page() -> Response {
        static_resource_response("datasource.html")
    }

    async fn sql_page() -> Response {
        static_resource_response("sql.html")
    }

    async fn sql_detail_page() -> Response {
        static_resource_response("sql-detail.html")
    }

    async fn wall_page() -> Response {
        static_resource_response("wall.html")
    }

    async fn api_page() -> Response {
        static_resource_response("api.html")
    }

    async fn spring_page() -> Response {
        static_resource_response("spring.html")
    }

    async fn spring_detail_page() -> Response {
        static_resource_response("spring-detail.html")
    }

    async fn weburi_page() -> Response {
        static_resource_response("weburi.html")
    }

    async fn weburi_detail_page() -> Response {
        static_resource_response("weburi-detail.html")
    }

    async fn websession_page() -> Response {
        static_resource_response("websession.html")
    }

    async fn websession_detail_page() -> Response {
        static_resource_response("websession-detail.html")
    }

    async fn webapp_page() -> Response {
        static_resource_response("webapp.html")
    }

    async fn connection_info_page() -> Response {
        static_resource_response("connectionInfo.html")
    }

    async fn active_connection_stack_trace_page() -> Response {
        static_resource_response("activeConnectionStackTrace.html")
    }

    async fn login_page() -> Response {
        static_resource_response("login.html")
    }

    async fn no_permit_page() -> Response {
        static_resource_response("nopermit.html")
    }

    async fn legacy_style(Path(asset): Path<String>) -> Response {
        static_resource_response(&format!("css/{asset}"))
    }

    async fn legacy_script(Path(asset): Path<String>) -> Response {
        static_resource_response(&format!("js/{asset}"))
    }

    async fn submit_login(
        State(state): State<Self>,
        Valid(Form(login)): Valid<Form<LoginRequest>>,
    ) -> Response {
        if !state
            .auth
            .authorizes_credentials(&login.login_username, &login.login_password)
        {
            return (StatusCode::OK, "error").into_response();
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        let now = Instant::now();
        let mut sessions = state.sessions.write();
        sessions.retain(|_, created| now.duration_since(*created) < SESSION_TTL);
        if sessions.len() >= MAX_SESSIONS {
            if let Some(oldest) = sessions
                .iter()
                .min_by_key(|(_, created)| **created)
                .map(|(token, _)| token.clone())
            {
                sessions.remove(&oldest);
            }
        }
        sessions.insert(token.clone(), now);
        drop(sessions);
        let secure = if state.secure_cookies { "; Secure" } else { "" };
        (
            StatusCode::OK,
            [(
                header::SET_COOKIE,
                format!("druid-session={token}; Path=/{secure}; HttpOnly; SameSite=Lax"),
            )],
            "success",
        )
            .into_response()
    }

    async fn service_list(State(state): State<Self>) -> Json<Vec<String>> {
        match &state.backend {
            MonitorBackend::Remote(service) => Json(service.applications().to_vec()),
            MonitorBackend::Repository(_) => Json(Vec::new()),
        }
    }

    async fn data_sources(State(state): State<Self>) -> Response {
        match &state.backend {
            MonitorBackend::Remote(service) => {
                let service = Arc::clone(service);
                state
                    .task_monitor
                    .instrument(async move {
                        match service.get_data_source_stat_data().await {
                            Ok(result) => Json(result).into_response(),
                            Err(error) => error_response(error),
                        }
                    })
                    .await
            }
            MonitorBackend::Repository(repository) => Json(DataSourceResult {
                result_code: MonitorStatService::RESULT_CODE_SUCCESS,
                content: Some(
                    repository
                        .all_datasources()
                        .into_iter()
                        .map(|entry| entry.datasource)
                        .collect(),
                ),
            })
            .into_response(),
        }
    }

    async fn data_source(
        State(state): State<Self>,
        Path(id): Path<i64>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId") else {
            return missing_parameter("serviceId");
        };
        let Ok(service_identity) = ServiceIdentity::parse(service_id) else {
            return invalid_service_id();
        };
        match &state.backend {
            MonitorBackend::Remote(_) => Self::data_sources(State(state)).await,
            MonitorBackend::Repository(repository) => {
                let content = repository
                    .get_datasource(&DataSourceId {
                        service_identity,
                        identity: id,
                    })
                    .map(|entry| entry.datasource);
                match content {
                    Some(content) => Json(DataSourceResult {
                        result_code: MonitorStatService::RESULT_CODE_SUCCESS,
                        content: Some(vec![content]),
                    })
                    .into_response(),
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        }
    }

    async fn sql_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Response {
        match &state.backend {
            MonitorBackend::Remote(service) => {
                let service = Arc::clone(service);
                state
                    .task_monitor
                    .instrument(async move {
                        match service.get_sql_stat_data_list(&query).await {
                            Ok(result) => Json(result).into_response(),
                            Err(error) => error_response(error),
                        }
                    })
                    .await
            }
            MonitorBackend::Repository(repository) => Json(json!({
                "ResultCode": MonitorStatService::RESULT_CODE_SUCCESS,
                "Content": repository.all_sql_stats(),
            }))
            .into_response(),
        }
    }

    async fn wall(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Response {
        match &state.backend {
            MonitorBackend::Remote(service) => {
                let service = Arc::clone(service);
                state
                    .task_monitor
                    .instrument(async move {
                        match service.get_wall_stat_map(&query).await {
                            Ok(result) => Json(result).into_response(),
                            Err(error) => error_response(error),
                        }
                    })
                    .await
            }
            MonitorBackend::Repository(repository) => {
                let mut wall = repository.aggregated_wall();
                wall.result_code = MonitorStatService::RESULT_CODE_SUCCESS;
                Json(wall).into_response()
            }
        }
    }

    async fn api_description() -> Json<Value> {
        Json(json!({
            "ResultCode": MonitorStatService::RESULT_CODE_SUCCESS,
            "Content": {
                "endpoints": serde_json::from_str::<Value>(Self::endpoint_list())
                    .unwrap_or_else(|_| json!([])),
                "version": "standalone-v1",
            }
        }))
    }

    async fn web_uri_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Response {
        let MonitorBackend::Remote(service) = &state.backend else {
            return Json(empty_success()).into_response();
        };
        let service = Arc::clone(service);
        state
            .task_monitor
            .instrument(async move {
                match service.get_web_uri_stat_data_list(&query).await {
                    Ok(result) => Json(result).into_response(),
                    Err(error) => error_response(error),
                }
            })
            .await
    }

    async fn web_uri_detail(State(state): State<Self>, Path(uri): Path<String>) -> Json<Value> {
        let content = match &state.backend {
            MonitorBackend::Remote(service) => service.get_web_uri_stat_data(&uri),
            MonitorBackend::Repository(_) => None,
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    async fn web_app_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        let content = match &state.backend {
            MonitorBackend::Remote(service) => service.get_web_app_stat_data_list(&query),
            MonitorBackend::Repository(_) => Vec::new(),
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    async fn web_session_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        let content = match &state.backend {
            MonitorBackend::Remote(service) => service.get_web_session_stat_data_list(&query),
            MonitorBackend::Repository(_) => Vec::new(),
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    async fn web_session_detail(State(state): State<Self>, Path(id): Path<String>) -> Json<Value> {
        let content = match &state.backend {
            MonitorBackend::Remote(service) => service.get_web_session_stat_data(&id),
            MonitorBackend::Repository(_) => None,
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    async fn spring_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        let content = match &state.backend {
            MonitorBackend::Remote(service) => service.get_spring_stat_data_list(&query),
            MonitorBackend::Repository(_) => Vec::new(),
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    async fn spring_detail(
        State(state): State<Self>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(class) = parameters.get("class") else {
            return missing_parameter("class");
        };
        let Some(method) = parameters.get("method") else {
            return missing_parameter("method");
        };
        let content = match &state.backend {
            MonitorBackend::Remote(service) => service.get_spring_method_stat_data(class, method),
            MonitorBackend::Repository(_) => None,
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
        .into_response()
    }

    async fn sql_detail(
        State(state): State<Self>,
        Path(id): Path<i64>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId") else {
            return missing_parameter("serviceId");
        };
        let Ok(service_identity) = ServiceIdentity::parse(service_id) else {
            return invalid_service_id();
        };
        let Some(datasource_id) = parameters
            .get("dataSourceId")
            .and_then(|value| value.parse::<i64>().ok())
        else {
            return missing_parameter("dataSourceId");
        };
        match &state.backend {
            MonitorBackend::Remote(service) => {
                let service = Arc::clone(service);
                state
                    .task_monitor
                    .instrument(async move {
                        match service.get_sql_stat(id, service_id).await {
                            Ok(result) => Json(result).into_response(),
                            Err(error) => error_response(error),
                        }
                    })
                    .await
            }
            MonitorBackend::Repository(repository) => repository
                .get_sql_stat(&service_identity, datasource_id, id)
                .map_or_else(
                    || StatusCode::NOT_FOUND.into_response(),
                    |stat| Json(stat).into_response(),
                ),
        }
    }

    async fn connection_info(
        State(state): State<Self>,
        Path(id): Path<i64>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId") else {
            return missing_parameter("serviceId");
        };
        let Ok(service_identity) = ServiceIdentity::parse(service_id) else {
            return invalid_service_id();
        };
        connection_response(state, id, service_identity).await
    }

    async fn active(
        State(state): State<Self>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId") else {
            return missing_parameter("serviceId");
        };
        let Ok(service_identity) = ServiceIdentity::parse(service_id) else {
            return invalid_service_id();
        };
        let Some(id) = parameters
            .get("id")
            .and_then(|value| value.parse::<i64>().ok())
        else {
            return missing_parameter("id");
        };
        connection_response(state, id, service_identity).await
    }

    async fn legacy_dispatch(
        State(state): State<Self>,
        OriginalUri(original_uri): OriginalUri,
    ) -> Response {
        let request_target = original_uri
            .path_and_query()
            .map_or(original_uri.path(), |path_and_query| {
                path_and_query.as_str()
            });
        let Some(service_url) = request_target.strip_prefix(state.context_path.as_str()) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        if !service_url.contains(".json") {
            return StatusCode::NOT_FOUND.into_response();
        }
        match &state.backend {
            MonitorBackend::Remote(service) => match service.service(service_url).await {
                Ok(body) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/json;charset=utf-8")],
                    body,
                )
                    .into_response(),
                Err(error) => error_response(error),
            },
            MonitorBackend::Repository(_) => Json(json!({
                "ResultCode": MonitorStatService::RESULT_CODE_ERROR,
                "Content": "Do not support this request, please contact with administrator.",
            }))
            .into_response(),
        }
    }

    async fn metrics(State(state): State<Self>) -> Response {
        let metrics = state.task_monitor.cumulative();
        #[allow(unused_mut)]
        let mut body = format!(
            concat!(
                "# HELP druid_admin_tasks_instrumented_total Total instrumented admin tasks.\n",
                "# TYPE druid_admin_tasks_instrumented_total counter\n",
                "druid_admin_tasks_instrumented_total {}\n",
                "# HELP druid_admin_tasks_completed_total Total completed admin tasks.\n",
                "# TYPE druid_admin_tasks_completed_total counter\n",
                "druid_admin_tasks_completed_total {}\n",
                "# HELP druid_admin_task_polls_total Total admin task polls.\n",
                "# TYPE druid_admin_task_polls_total counter\n",
                "druid_admin_task_polls_total {}\n",
                "# HELP druid_admin_task_slow_polls_total Total slow admin task polls.\n",
                "# TYPE druid_admin_task_slow_polls_total counter\n",
                "druid_admin_task_slow_polls_total {}\n"
            ),
            metrics.instrumented_count,
            metrics.dropped_count,
            metrics.total_poll_count,
            metrics.total_slow_poll_count
        );
        if let MonitorBackend::Repository(repository) = &state.backend {
            write_repository_metrics(&mut body, repository);
        }
        #[cfg(feature = "jdbc-agent")]
        {
            let agent = druid_wrapper::jdbc_agent::JdbcAgentRuntimeMetrics::snapshot();
            write!(
                &mut body,
                concat!(
                    "# HELP druid_jdbc_agent_processes Current JDBC agent process count.\n",
                    "# TYPE druid_jdbc_agent_processes gauge\n",
                    "druid_jdbc_agent_processes {}\n",
                    "# HELP druid_jdbc_agent_active_sessions Current JDBC agent active sessions.\n",
                    "# TYPE druid_jdbc_agent_active_sessions gauge\n",
                    "druid_jdbc_agent_active_sessions {}\n",
                    "# HELP druid_jdbc_agent_starts_total Total JDBC agent starts.\n",
                    "# TYPE druid_jdbc_agent_starts_total counter\n",
                    "druid_jdbc_agent_starts_total {}\n",
                    "# HELP druid_jdbc_agent_crashes_total Total JDBC agent crashes.\n",
                    "# TYPE druid_jdbc_agent_crashes_total counter\n",
                    "druid_jdbc_agent_crashes_total {}\n",
                    "# HELP druid_jdbc_agent_rpc_total Total JDBC agent RPC calls.\n",
                    "# TYPE druid_jdbc_agent_rpc_total counter\n",
                    "druid_jdbc_agent_rpc_total {}\n",
                    "# HELP druid_jdbc_agent_rpc_errors_total Total JDBC agent RPC errors.\n",
                    "# TYPE druid_jdbc_agent_rpc_errors_total counter\n",
                    "druid_jdbc_agent_rpc_errors_total {}\n",
                    "# HELP druid_jdbc_agent_rpc_latency_microseconds_total Total JDBC agent RPC latency in microseconds.\n",
                    "# TYPE druid_jdbc_agent_rpc_latency_microseconds_total counter\n",
                    "druid_jdbc_agent_rpc_latency_microseconds_total {}\n",
                    "# HELP druid_jdbc_agent_rpc_latency_microseconds_max Maximum JDBC agent RPC latency in microseconds.\n",
                    "# TYPE druid_jdbc_agent_rpc_latency_microseconds_max gauge\n",
                    "druid_jdbc_agent_rpc_latency_microseconds_max {}\n",
                    "# HELP druid_jdbc_agent_timeouts_total Total JDBC agent RPC timeouts.\n",
                    "# TYPE druid_jdbc_agent_timeouts_total counter\n",
                    "druid_jdbc_agent_timeouts_total {}\n",
                    "# HELP druid_jdbc_agent_cancellations_total Total JDBC agent RPC cancellations.\n",
                    "# TYPE druid_jdbc_agent_cancellations_total counter\n",
                    "druid_jdbc_agent_cancellations_total {}\n",
                    "# HELP druid_jdbc_agent_protocol_errors_total Total JDBC agent protocol errors.\n",
                    "# TYPE druid_jdbc_agent_protocol_errors_total counter\n",
                    "druid_jdbc_agent_protocol_errors_total {}\n"
                ),
                agent.process_count(),
                agent.active_sessions(),
                agent.start_count(),
                agent.crash_count(),
                agent.rpc_count(),
                agent.rpc_error_count(),
                agent.rpc_latency_micros_total(),
                agent.rpc_latency_micros_max(),
                agent.timeout_count(),
                agent.cancellation_count(),
                agent.protocol_error_count(),
            )
            .expect("writing metrics into String cannot fail");
        }
        if let MonitorBackend::Repository(repository) = &state.backend {
            if !repository.accept_metrics_output_size(body.len()) {
                let (_, _, output_rejected_total) = repository.limit_counters();
                let rejected = format!(
                    concat!(
                        "# HELP druid_admin_metrics_output_rejected_total Total metrics expositions rejected for exceeding the byte budget.\n",
                        "# TYPE druid_admin_metrics_output_rejected_total counter\n",
                        "druid_admin_metrics_output_rejected_total {}\n",
                        "# HELP druid_admin_metrics_output_limit_bytes Configured maximum metrics exposition bytes.\n",
                        "# TYPE druid_admin_metrics_output_limit_bytes gauge\n",
                        "druid_admin_metrics_output_limit_bytes {}\n"
                    ),
                    output_rejected_total,
                    repository.max_metrics_output_bytes()
                );
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
                    rejected,
                )
                    .into_response();
            }
        }
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            body,
        )
            .into_response()
    }
}

fn required_json_string<'a>(body: &'a Value, field: &str) -> Result<&'a str, String> {
    body.get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{field} must be a non-empty string"))
}

fn command_error_status(error: &str) -> StatusCode {
    if error.contains("conflicting canonical payload") {
        StatusCode::CONFLICT
    } else if error.contains("queue full") || error.contains("broker full") {
        StatusCode::TOO_MANY_REQUESTS
    } else if error.contains("deadline exceeded") {
        StatusCode::GATEWAY_TIMEOUT
    } else if error.contains("no live stream") {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::BAD_REQUEST
    }
}

fn command_error_response(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({"success": false, "error": error}))).into_response()
}

async fn connection_response(
    state: MonitorViewServlet,
    id: i64,
    service_identity: ServiceIdentity,
) -> Response {
    match &state.backend {
        MonitorBackend::Remote(service) => {
            let service = Arc::clone(service);
            state
                .task_monitor
                .instrument(async move {
                    match service
                        .get_pooling_connection_info_by_data_source_id(
                            id,
                            service_identity.service_id(),
                        )
                        .await
                    {
                        Ok(result) => Json(result).into_response(),
                        Err(error) => error_response(error),
                    }
                })
                .await
        }
        MonitorBackend::Repository(repository) => repository
            .get_datasource(&DataSourceId {
                service_identity,
                identity: id,
            })
            .map_or_else(
                || StatusCode::NOT_FOUND.into_response(),
                |entry| Json(entry.datasource).into_response(),
            ),
    }
}

fn empty_success() -> Value {
    json!({
        "ResultCode": MonitorStatService::RESULT_CODE_SUCCESS,
        "Content": [],
    })
}

fn write_repository_metrics(body: &mut String, repository: &MetricsRepository) {
    let (ingest_total, rejected_total) = repository.counters();
    let (capacity_rejected_total, ttl_evicted_total, output_rejected_total) =
        repository.limit_counters();
    let datasource_count = repository.len();
    let _ = writeln!(
        body,
        "# HELP druid_admin_ingest_total Total number of ingest batches received.\n# TYPE druid_admin_ingest_total counter\ndruid_admin_ingest_total {ingest_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_capacity_rejected_total Total repository transactions rejected by configured capacity.\n# TYPE druid_admin_capacity_rejected_total counter\ndruid_admin_capacity_rejected_total {capacity_rejected_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_ttl_evicted_total Total datasource entries evicted after TTL.\n# TYPE druid_admin_ttl_evicted_total counter\ndruid_admin_ttl_evicted_total {ttl_evicted_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_metrics_output_rejected_total Total metrics expositions rejected for exceeding the byte budget.\n# TYPE druid_admin_metrics_output_rejected_total counter\ndruid_admin_metrics_output_rejected_total {output_rejected_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_metrics_output_limit_bytes Configured maximum metrics exposition bytes.\n# TYPE druid_admin_metrics_output_limit_bytes gauge\ndruid_admin_metrics_output_limit_bytes {}",
        repository.max_metrics_output_bytes()
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_ingest_rejected_total Total number of rejected ingest batches.\n# TYPE druid_admin_ingest_rejected_total counter\ndruid_admin_ingest_rejected_total {rejected_total}"
    );
    let _ = writeln!(
        body,
        "# HELP druid_admin_datasource_count Number of tracked datasource instances.\n# TYPE druid_admin_datasource_count gauge\ndruid_admin_datasource_count {datasource_count}"
    );
    body.push_str(concat!(
        "# HELP druid_admin_datasource_active_count Active connection count.\n",
        "# TYPE druid_admin_datasource_active_count gauge\n",
        "# HELP druid_admin_datasource_pooling_count Pooled connection count.\n",
        "# TYPE druid_admin_datasource_pooling_count gauge\n",
        "# HELP druid_admin_datasource_execute_count Total execute count.\n",
        "# TYPE druid_admin_datasource_execute_count counter\n",
        "# HELP druid_admin_datasource_error_count Total error count.\n",
        "# TYPE druid_admin_datasource_error_count counter\n",
        "# HELP druid_admin_datasource_commit_count Total commit count.\n",
        "# TYPE druid_admin_datasource_commit_count counter\n",
        "# HELP druid_admin_datasource_rollback_count Total rollback count.\n",
        "# TYPE druid_admin_datasource_rollback_count counter\n",
    ));
    for entry in repository.all_datasources() {
        let datasource = &entry.datasource;
        let service_id =
            prometheus_label_value(datasource.service_id.as_deref().unwrap_or("unknown"));
        let service_name =
            prometheus_label_value(datasource.service_name.as_deref().unwrap_or("unknown"));
        let instance_id =
            prometheus_label_value(datasource.instance_id.as_deref().unwrap_or("unknown"));
        let boot_id = prometheus_label_value(datasource.boot_id.as_deref().unwrap_or("unknown"));
        let datasource_name =
            prometheus_label_value(datasource.name.as_deref().unwrap_or("unknown"));
        let db_type = prometheus_label_value(datasource.db_type.as_deref().unwrap_or("unknown"));
        let identity = prometheus_label_value(&datasource.identity.to_string());
        let labels = format!(
            "service_id=\"{service_id}\",service_name=\"{service_name}\",instance_id=\"{instance_id}\",boot_id=\"{boot_id}\",datasource=\"{identity}\",datasource_name=\"{datasource_name}\",db_type=\"{db_type}\""
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_active_count{{{labels}}} {}",
            datasource.active_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_pooling_count{{{labels}}} {}",
            datasource.pooling_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_execute_count{{{labels}}} {}",
            datasource.execute_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_error_count{{{labels}}} {}",
            datasource.error_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_commit_count{{{labels}}} {}",
            datasource.commit_count
        );
        let _ = writeln!(
            body,
            "druid_admin_datasource_rollback_count{{{labels}}} {}",
            datasource.rollback_count
        );
    }
}

fn prometheus_label_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            other => escaped.push(other),
        }
    }
    escaped
}

fn missing_parameter(name: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"ResultCode": -1, "Content": format!("missing parameter: {name}")})),
    )
        .into_response()
}

fn invalid_service_id() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"ResultCode": -1, "Content": "invalid canonical serviceId"})),
    )
        .into_response()
}

fn error_response(error: impl std::fmt::Display) -> Response {
    let error_msg = error.to_string();
    // Map well-known error patterns to appropriate HTTP status codes.
    let status = if error_msg.contains("unknown serviceId") {
        StatusCode::NOT_FOUND
    } else if error_msg.contains("invalid parameter") {
        StatusCode::UNPROCESSABLE_ENTITY
    } else {
        StatusCode::BAD_GATEWAY
    };
    (
        status,
        Json::<Value>(json!({"ResultCode": -1, "Content": error_msg})),
    )
        .into_response()
}

fn redirect_found(location: String) -> Response {
    (StatusCode::FOUND, [(header::LOCATION, location)]).into_response()
}

fn is_public_path(path: &str, context_path: &str) -> bool {
    let Some(relative_path) = path.strip_prefix(context_path) else {
        return false;
    };
    matches!(
        relative_path,
        "/login.html" | "/nopermit.html" | "/submitLogin"
    ) || relative_path.starts_with("/css/")
        || relative_path.starts_with("/js/")
}

fn is_command_path(path: &str, context_path: &str) -> bool {
    path.strip_prefix(context_path) == Some("/api/commands")
}

/// Check whether the request carries a valid (non-expired) session token.
///
/// While holding the write lock, expired sessions are pruned; admission also
/// enforces a hard cap independent of login rate.
fn request_has_session(request: &Request, sessions: &RwLock<HashMap<String, Instant>>) -> bool {
    let token = request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == "druid-session").then_some(value.to_owned())
            })
        });
    let Some(token) = token else {
        return false;
    };
    let now = Instant::now();
    let mut guard = sessions.write();
    // Prune expired sessions.
    guard.retain(|_, created| now.duration_since(*created) < SESSION_TTL);
    guard
        .get(&token)
        .is_some_and(|created| now.duration_since(*created) < SESSION_TTL)
}

fn static_resource_response(resource: &str) -> Response {
    let Some(body) = static_resource(resource) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = if resource.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if resource.ends_with(".css") {
        "text/css;charset=utf-8"
    } else {
        "text/javascript;charset=utf-8"
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "max-age=36000"),
        ],
        body,
    )
        .into_response()
}

fn static_resource(resource: &str) -> Option<&'static str> {
    Some(match resource {
        "activeConnectionStackTrace.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/activeConnectionStackTrace.html"
        )),
        "api.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/api.html"
        )),
        "connectionInfo.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/connectionInfo.html"
        )),
        "datasource.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/datasource.html"
        )),
        "header.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/header.html"
        )),
        "index.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/index.html"
        )),
        "login.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/login.html"
        )),
        "nopermit.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/nopermit.html"
        )),
        "spring-detail.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/spring-detail.html"
        )),
        "spring.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/spring.html"
        )),
        "sql-detail.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/sql-detail.html"
        )),
        "sql.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/sql.html"
        )),
        "wall.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/wall.html"
        )),
        "webapp.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/webapp.html"
        )),
        "websession-detail.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/websession-detail.html"
        )),
        "websession.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/websession.html"
        )),
        "weburi-detail.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/weburi-detail.html"
        )),
        "weburi.html" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/weburi.html"
        )),
        "css/bootstrap.min.css" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/css/bootstrap.min.css"
        )),
        "css/style.css" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/css/style.css"
        )),
        "js/bootstrap.min.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/bootstrap.min.js"
        )),
        "js/common.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/common.js"
        )),
        "js/doT.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/doT.js"
        )),
        "js/jquery.min.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/jquery.min.js"
        )),
        "js/lang.js" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/support/http/resources/js/lang.js"
        )),
        _ => return None,
    })
}
