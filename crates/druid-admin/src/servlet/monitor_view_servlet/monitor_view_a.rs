use super::*;

impl MonitorViewServlet {
    /// 创建管理路由对象。
    #[must_use]
    pub fn new(monitor_stat_service: Arc<MonitorStatService>) -> Self {
        Self {
            backend: MonitorBackend {
                source: Arc::new(ManagementSource::remote(monitor_stat_service)),
            },
            task_monitor: TaskMonitor::new(),
            auth: AdminAuthMode::Disabled,
            redaction_policy: AdminRedactionPolicy::compatibility(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            context_path: Arc::new("/druid".to_owned()),
            secure_cookies: false,
            command_broker: None,
            metrics_appendix: None,
            remote_metrics_output_rejected_total: Arc::new(AtomicU64::new(0)),
            bearer_token: None,
            cors_origins: Arc::new(Vec::new()),
        }
    }

    /// 使用统一的来源创建管理路由；管理端点不感知数据来源。
    #[must_use]
    pub fn from_management_source(source: ManagementSource) -> Self {
        Self {
            backend: MonitorBackend {
                source: Arc::new(source),
            },
            task_monitor: TaskMonitor::new(),
            auth: AdminAuthMode::Disabled,
            redaction_policy: AdminRedactionPolicy::compatibility(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            context_path: Arc::new("/druid".to_owned()),
            secure_cookies: false,
            command_broker: None,
            metrics_appendix: None,
            remote_metrics_output_rejected_total: Arc::new(AtomicU64::new(0)),
            bearer_token: None,
            cors_origins: Arc::new(Vec::new()),
        }
    }

    /// 使用 Metrics gRPC ingest 的本地 repository 创建 standalone 管理路由。
    #[must_use]
    pub fn from_repository(repository: MetricsRepository) -> Self {
        Self::from_management_source(ManagementSource::repository(repository))
    }

    /// 配置 Java `loginUsername`/`loginPassword` 登录语义。
    #[must_use]
    pub fn with_credentials(
        self,
        login_username: Option<String>,
        login_password: Option<String>,
    ) -> Self {
        let auth = login_username.map_or(AdminAuthMode::Disabled, |username| {
            AdminAuthMode::basic(username, login_password.unwrap_or_default())
        });
        self.with_auth_mode(auth)
    }

    /// 配置 HTTP 与 gRPC 共用的应用层认证模式。
    #[must_use]
    pub fn with_auth_mode(mut self, auth: AdminAuthMode) -> Self {
        if self.auth != auth {
            // 认证身份改变后不能让 clone 共享的旧会话在新路由继续生效。
            self.sessions = Arc::new(RwLock::new(HashMap::new()));
        }
        self.auth = auth;
        self.redaction_policy = if self.auth.is_enabled() {
            AdminRedactionPolicy::secure()
        } else {
            AdminRedactionPolicy::compatibility()
        };
        self
    }

    /// 注入宿主进程的可选 Prometheus 指标文本；返回值须已脱敏并使用合法 exposition 格式。
    /// 指标会计入现有输出容量预算，不引入对驱动实现 crate 的依赖。
    #[must_use]
    pub fn with_metrics_appendix(
        mut self,
        provider: impl Fn() -> String + Send + Sync + 'static,
    ) -> Self {
        self.metrics_appendix = Some(Arc::new(provider));
        self
    }

    /// 指定 session cookie 是否仅允许通过 TLS 发送。
    #[must_use]
    pub fn with_secure_cookies(mut self, secure_cookies: bool) -> Self {
        self.secure_cookies = secure_cookies;
        self
    }

    /// 配置 Bearer 令牌；跨源部署的控制台用它代替会话 cookie。
    #[must_use]
    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(Arc::from(token.into()));
        self
    }

    /// 配置允许的跨源来源；非空时给响应加 CORS 头。
    #[must_use]
    pub fn with_cors_origins(mut self, origins: Vec<String>) -> Self {
        self.cors_origins = Arc::new(origins);
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
        let redaction_policy = self.redaction_policy;
        let context_path = self.context_path.to_string();
        let cors_origins = self.cors_origins.clone();
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
            // 版本化管理 API：分离前端只读这一套。
            .route("/api/v1/basic", get(super::v1_api::basic))
            .route("/api/v1/datasources", get(super::v1_api::datasources))
            .route(
                "/api/v1/datasources/{identity}",
                get(super::v1_api::datasource),
            )
            .route("/api/v1/sql", get(super::v1_api::sql_list))
            .route("/api/v1/sql/{id}", get(super::v1_api::sql_detail))
            .route("/api/v1/wall", get(super::v1_api::wall))
            .route("/api/v1/services", get(super::v1_api::services))
            .route("/api/v1/openapi.json", get(super::v1_api::openapi))
            // 分离前端：独立构建的应用，资源内嵌在二进制中。
            .route("/console", get(Self::console_entry))
            .route("/console/", get(Self::console_entry))
            .route("/console/{*path}", get(Self::console_asset))
            .fallback(Self::legacy_dispatch);
        // Merge admin routes into a single router that also handles the
        // bare context-path (e.g. /druid and /druid/) by redirecting to
        // the index page.
        let context_slash = format!("{context_path}/");
        let mut router = Router::new()
            .route(&context_path, get(Self::root))
            .route(&context_slash, get(Self::root))
            .merge(
                Router::new()
                    .nest(&context_path, admin_routes)
                    .route("/metrics", get(Self::metrics)),
            )
            .with_state(self)
            .layer(middleware::from_fn_with_state(auth_state, Self::authorize))
            .layer(middleware::from_fn_with_state(
                redaction_policy,
                Self::redact_management_response,
            ));
        if !cors_origins.is_empty() {
            let origins: Vec<axum::http::HeaderValue> = cors_origins
                .iter()
                .filter_map(|origin| origin.parse::<axum::http::HeaderValue>().ok())
                .collect();
            let cors = tower_http::cors::CorsLayer::new()
                .allow_origin(tower_http::cors::AllowOrigin::list(origins))
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
                .allow_headers([
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::CONTENT_TYPE,
                ]);
            router = router.layer(cors);
        }
        router
    }

    pub(crate) async fn redact_management_response(
        State(policy): State<AdminRedactionPolicy>,
        request: Request,
        next: Next,
    ) -> Response {
        // 版本化 API 面向已认证的 operator，保留其需要的字段；
        // 只有 Java 兼容路由做保守脱敏。
        let path = request.uri().path().to_owned();
        let response = next.run(request).await;
        if path.contains("/api/v1/") {
            return response;
        }
        if !policy.is_enabled()
            || !response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("application/json"))
        {
            return response;
        }
        let (mut parts, body) = response.into_parts();
        let bytes = match to_bytes(body, MAX_MANAGEMENT_JSON_RESPONSE_BYTES).await {
            Ok(bytes) => bytes,
            Err(_) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(
                        json!({"ResultCode": -1, "Content": "admin JSON response exceeds budget"}),
                    ),
                )
                    .into_response();
            }
        };
        let mut json = match serde_json::from_slice::<Value>(&bytes) {
            Ok(json) => json,
            Err(_) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"ResultCode": -1, "Content": "invalid upstream admin JSON"})),
                )
                    .into_response();
            }
        };
        policy.redact_json(&mut json);
        let bytes = match serde_json::to_vec(&json) {
            Ok(bytes) => bytes,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"ResultCode": -1, "Content": "admin JSON serialization failed"})),
                )
                    .into_response();
            }
        };
        parts.headers.remove(header::CONTENT_LENGTH);
        Response::from_parts(parts, Body::from(bytes))
    }

    /// 返回稳定端点清单。
    #[must_use]
    pub fn endpoint_list() -> &'static str {
        r#"["/druid/api/datasources","/druid/api/sql/top","/druid/api/sql/slow","/druid/api/wall","/druid/api/active","/metrics"]"#
    }

    pub(crate) async fn authorize(
        State(state): State<Self>,
        mut request: Request,
        next: Next,
    ) -> Response {
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
        if is_public_path(request.uri().path(), state.context_path.as_str()) {
            return next.run(request).await;
        }
        if request_has_session(&request, &state.sessions)
            || state.auth.authorizes_header(authorization)
            || state.authorizes_bearer(authorization)
        {
            request
                .extensions_mut()
                .insert(crate::config::AdminRole::Operator);
            return next.run(request).await;
        }
        redirect_found(format!("{}/login.html", state.context_path))
    }

    /// 校验 Bearer 令牌；令牌按字节做常量时间比较。
    fn authorizes_bearer(&self, authorization: Option<&str>) -> bool {
        let Some(token) = &self.bearer_token else {
            return false;
        };
        let Some(value) = authorization else {
            return false;
        };
        let Some(provided) = value.strip_prefix("Bearer ") else {
            return false;
        };
        use subtle::ConstantTimeEq;
        let expected = token.as_bytes();
        let provided = provided.as_bytes();
        expected.len() == provided.len() && expected.ct_eq(provided).into()
    }

    pub(crate) async fn issue_command(
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
                    // CommandAck 来自远端客户端；错误文本可能携带 URL、用户名或密码。
                    "error": "command failed"
                })),
            )
                .into_response(),
            Err(error) => command_broker_error_response(&error),
        }
    }

    pub(crate) async fn root(State(state): State<Self>) -> Response {
        // 分离控制台是默认入口；未构建前端时回退到 legacy 页面。
        if super::console::entry_document().is_some() {
            redirect_found(format!("{}/console/", state.context_path))
        } else {
            redirect_found(format!("{}/index.html", state.context_path))
        }
    }

    pub(crate) async fn legacy_page() -> Response {
        static_resource_response("index.html")
    }

    pub(crate) async fn datasource_page() -> Response {
        static_resource_response("datasource.html")
    }

    pub(crate) async fn sql_page() -> Response {
        static_resource_response("sql.html")
    }

    pub(crate) async fn sql_detail_page() -> Response {
        static_resource_response("sql-detail.html")
    }

    pub(crate) async fn wall_page() -> Response {
        static_resource_response("wall.html")
    }

    pub(crate) async fn api_page() -> Response {
        static_resource_response("api.html")
    }

    pub(crate) async fn spring_page() -> Response {
        static_resource_response("spring.html")
    }

    pub(crate) async fn spring_detail_page() -> Response {
        static_resource_response("spring-detail.html")
    }

    pub(crate) async fn weburi_page() -> Response {
        static_resource_response("weburi.html")
    }

    pub(crate) async fn weburi_detail_page() -> Response {
        static_resource_response("weburi-detail.html")
    }

    pub(crate) async fn websession_page() -> Response {
        static_resource_response("websession.html")
    }

    pub(crate) async fn websession_detail_page() -> Response {
        static_resource_response("websession-detail.html")
    }

    pub(crate) async fn webapp_page() -> Response {
        static_resource_response("webapp.html")
    }

    pub(crate) async fn connection_info_page() -> Response {
        static_resource_response("connectionInfo.html")
    }

    pub(crate) async fn active_connection_stack_trace_page() -> Response {
        static_resource_response("activeConnectionStackTrace.html")
    }

    pub(crate) async fn login_page() -> Response {
        static_resource_response("login.html")
    }

    pub(crate) async fn no_permit_page() -> Response {
        static_resource_response("nopermit.html")
    }

    /// 分离前端入口。
    pub(crate) async fn console_entry() -> Response {
        let Some(body) = super::console::entry_document() else {
            return (
                StatusCode::NOT_FOUND,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                "the console application has not been built; see crates/druid-admin/web",
            )
                .into_response();
        };
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            body,
        )
            .into_response()
    }

    /// 分离前端静态资源。
    pub(crate) async fn console_asset(Path(path): Path<String>) -> Response {
        super::console::serve(&path)
    }

    pub(crate) async fn legacy_style(Path(asset): Path<String>) -> Response {
        static_resource_response(&format!("css/{asset}"))
    }

    pub(crate) async fn legacy_script(Path(asset): Path<String>) -> Response {
        static_resource_response(&format!("js/{asset}"))
    }
}
