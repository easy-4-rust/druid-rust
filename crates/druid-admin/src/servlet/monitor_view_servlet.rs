#![allow(clippy::case_sensitive_file_extension_comparisons)]
#![allow(clippy::unused_async)] // axum handlers require async signature even when no .await

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::extract::{Form, OriginalUri, Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_valid::Valid;
use parking_lot::RwLock;
use serde_json::{json, Value};
#[cfg(feature = "jdbc-agent")]
use std::fmt::Write;
use tokio_metrics::TaskMonitor;

use crate::service::{MonitorStatService, StatQuery};

use super::LoginRequest;

/// Druid 管理协议的 Axum 路由。
///
/// 对应 Java: `com.alibaba.druid.admin.servlet.MonitorViewServlet`。
#[derive(Clone)]
pub struct MonitorViewServlet {
    monitor_stat_service: Arc<MonitorStatService>,
    task_monitor: TaskMonitor,
    credentials: Option<Arc<(String, String)>>,
    sessions: Arc<RwLock<HashSet<String>>>,
    context_path: Arc<String>,
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
            monitor_stat_service,
            task_monitor: TaskMonitor::new(),
            credentials: None,
            sessions: Arc::new(RwLock::new(HashSet::new())),
            context_path: Arc::new("/druid".to_owned()),
        }
    }

    /// 配置 Java `loginUsername`/`loginPassword` 登录语义。
    #[must_use]
    pub fn with_credentials(
        mut self,
        login_username: Option<String>,
        login_password: Option<String>,
    ) -> Self {
        self.credentials =
            login_username.map(|username| Arc::new((username, login_password.unwrap_or_default())));
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
            .route("/", get(Self::root))
            .route("/index.html", get(Self::legacy_page))
            .route("/login.html", get(Self::login_page))
            .route("/nopermit.html", get(Self::no_permit_page))
            .route("/{page}.html", get(Self::named_legacy_page))
            .route("/css/{asset}", get(Self::legacy_style))
            .route("/js/{asset}", get(Self::legacy_script))
            .route("/submitLogin", post(Self::submit_login))
            .route("/serviceList.json", get(Self::service_list))
            .route("/datasource.json", get(Self::data_sources))
            .route("/sql.json", get(Self::sql_list))
            .route("/wall.json", get(Self::wall))
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
            .fallback(Self::legacy_dispatch);
        Router::new()
            .route(&context_path, get(Self::root))
            .nest(&context_path, admin_routes)
            .route("/metrics", get(Self::metrics))
            .with_state(self)
            .layer(middleware::from_fn_with_state(auth_state, Self::authorize))
    }

    /// 返回稳定端点清单。
    #[must_use]
    pub fn endpoint_list() -> &'static str {
        r#"["/druid/api/datasources","/druid/api/sql/top","/druid/api/sql/slow","/druid/api/wall","/druid/api/active","/metrics"]"#
    }

    async fn authorize(State(state): State<Self>, request: Request, next: Next) -> Response {
        let Some(credentials) = state.credentials.as_ref() else {
            return next.run(request).await;
        };
        if is_public_path(request.uri().path(), state.context_path.as_str())
            || request_has_session(&request, &state.sessions)
            || request_has_credentials(&request, credentials)
        {
            return next.run(request).await;
        }
        redirect_found(format!("{}/login.html", state.context_path))
    }

    async fn root(State(state): State<Self>) -> Response {
        redirect_found(format!("{}/index.html", state.context_path))
    }

    async fn legacy_page() -> Response {
        static_resource_response("index.html")
    }

    async fn named_legacy_page(Path(page): Path<String>) -> Response {
        static_resource_response(&format!("{page}.html"))
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
        let Some(credentials) = state.credentials.as_ref() else {
            return (StatusCode::OK, "error").into_response();
        };
        if login.login_username != credentials.0 || login.login_password != credentials.1 {
            return (StatusCode::OK, "error").into_response();
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        state.sessions.write().insert(token.clone());
        (
            StatusCode::OK,
            [(
                header::SET_COOKIE,
                format!("druid-session={token}; Path=/; HttpOnly; SameSite=Lax"),
            )],
            "success",
        )
            .into_response()
    }

    async fn service_list(State(state): State<Self>) -> Json<Vec<String>> {
        Json(state.monitor_stat_service.applications().to_vec())
    }

    async fn data_sources(State(state): State<Self>) -> Response {
        let service = Arc::clone(&state.monitor_stat_service);
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

    async fn data_source(State(state): State<Self>, Path(_id): Path<i64>) -> Response {
        Self::data_sources(State(state)).await
    }

    async fn sql_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Response {
        let service = Arc::clone(&state.monitor_stat_service);
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

    async fn wall(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Response {
        let service = Arc::clone(&state.monitor_stat_service);
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

    async fn web_uri_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Response {
        let service = Arc::clone(&state.monitor_stat_service);
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
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            state.monitor_stat_service.get_web_uri_stat_data(&uri),
        ))
    }

    async fn web_app_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            state
                .monitor_stat_service
                .get_web_app_stat_data_list(&query),
        ))
    }

    async fn web_session_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            state
                .monitor_stat_service
                .get_web_session_stat_data_list(&query),
        ))
    }

    async fn web_session_detail(State(state): State<Self>, Path(id): Path<String>) -> Json<Value> {
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            state.monitor_stat_service.get_web_session_stat_data(&id),
        ))
    }

    async fn spring_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            state.monitor_stat_service.get_spring_stat_data_list(&query),
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
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            state
                .monitor_stat_service
                .get_spring_method_stat_data(class, method),
        ))
        .into_response()
    }

    async fn sql_detail(
        State(state): State<Self>,
        Path(id): Path<i64>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId").cloned() else {
            return missing_parameter("serviceId");
        };
        let service = Arc::clone(&state.monitor_stat_service);
        state
            .task_monitor
            .instrument(async move {
                match service.get_sql_stat(id, &service_id).await {
                    Ok(result) => Json(result).into_response(),
                    Err(error) => error_response(error),
                }
            })
            .await
    }

    async fn connection_info(
        State(state): State<Self>,
        Path(id): Path<i64>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId").cloned() else {
            return missing_parameter("serviceId");
        };
        connection_response(state, id, service_id).await
    }

    async fn active(
        State(state): State<Self>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId").cloned() else {
            return missing_parameter("serviceId");
        };
        let Some(id) = parameters
            .get("id")
            .and_then(|value| value.parse::<i64>().ok())
        else {
            return missing_parameter("id");
        };
        connection_response(state, id, service_id).await
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
        match state.monitor_stat_service.service(service_url).await {
            Ok(body) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json;charset=utf-8")],
                body,
            )
                .into_response(),
            Err(error) => error_response(error),
        }
    }

    async fn metrics(State(state): State<Self>) -> Response {
        let metrics = state.task_monitor.cumulative();
        #[allow(unused_mut)]
        let mut body = format!(
            concat!(
                "# TYPE druid_admin_tasks_instrumented_total counter\n",
                "druid_admin_tasks_instrumented_total {}\n",
                "# TYPE druid_admin_tasks_completed_total counter\n",
                "druid_admin_tasks_completed_total {}\n",
                "# TYPE druid_admin_task_polls_total counter\n",
                "druid_admin_task_polls_total {}\n",
                "# TYPE druid_admin_task_slow_polls_total counter\n",
                "druid_admin_task_slow_polls_total {}\n"
            ),
            metrics.instrumented_count,
            metrics.dropped_count,
            metrics.total_poll_count,
            metrics.total_slow_poll_count
        );
        #[cfg(feature = "jdbc-agent")]
        {
            let agent = druid_wrapper::jdbc_agent::JdbcAgentRuntimeMetrics::snapshot();
            write!(
                &mut body,
                concat!(
                    "# TYPE druid_jdbc_agent_processes gauge\n",
                    "druid_jdbc_agent_processes {}\n",
                    "# TYPE druid_jdbc_agent_active_sessions gauge\n",
                    "druid_jdbc_agent_active_sessions {}\n",
                    "# TYPE druid_jdbc_agent_starts_total counter\n",
                    "druid_jdbc_agent_starts_total {}\n",
                    "# TYPE druid_jdbc_agent_crashes_total counter\n",
                    "druid_jdbc_agent_crashes_total {}\n",
                    "# TYPE druid_jdbc_agent_rpc_total counter\n",
                    "druid_jdbc_agent_rpc_total {}\n",
                    "# TYPE druid_jdbc_agent_rpc_errors_total counter\n",
                    "druid_jdbc_agent_rpc_errors_total {}\n",
                    "# TYPE druid_jdbc_agent_rpc_latency_microseconds_total counter\n",
                    "druid_jdbc_agent_rpc_latency_microseconds_total {}\n",
                    "# TYPE druid_jdbc_agent_rpc_latency_microseconds_max gauge\n",
                    "druid_jdbc_agent_rpc_latency_microseconds_max {}\n",
                    "# TYPE druid_jdbc_agent_timeouts_total counter\n",
                    "druid_jdbc_agent_timeouts_total {}\n",
                    "# TYPE druid_jdbc_agent_cancellations_total counter\n",
                    "druid_jdbc_agent_cancellations_total {}\n",
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
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            body,
        )
            .into_response()
    }
}

async fn connection_response(state: MonitorViewServlet, id: i64, service_id: String) -> Response {
    let service = Arc::clone(&state.monitor_stat_service);
    state
        .task_monitor
        .instrument(async move {
            match service
                .get_pooling_connection_info_by_data_source_id(id, &service_id)
                .await
            {
                Ok(result) => Json(result).into_response(),
                Err(error) => error_response(error),
            }
        })
        .await
}

fn missing_parameter(name: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"ResultCode": -1, "Content": format!("missing parameter: {name}")})),
    )
        .into_response()
}

fn error_response(error: impl std::fmt::Display) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json::<Value>(json!({"ResultCode": -1, "Content": error.to_string()})),
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

fn request_has_session(request: &Request, sessions: &RwLock<HashSet<String>>) -> bool {
    request
        .headers()
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == "druid-session").then_some(value)
            })
        })
        .is_some_and(|token| sessions.read().contains(token))
}

fn request_has_credentials(request: &Request, credentials: &(String, String)) -> bool {
    let parameters: HashMap<_, _> = request
        .uri()
        .query()
        .unwrap_or_default()
        .split('&')
        .filter_map(|parameter| parameter.split_once('='))
        .collect();
    parameters.get("loginUsername") == Some(&credentials.0.as_str())
        && parameters.get("loginPassword") == Some(&credentials.1.as_str())
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
