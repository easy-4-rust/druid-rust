use super::*;

impl MonitorViewServlet {
    pub(crate) async fn submit_login(
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

    pub(crate) async fn service_list(State(state): State<Self>) -> Json<Vec<String>> {
        Json(state.backend.source().services())
    }

    pub(crate) async fn data_sources(State(state): State<Self>) -> Response {
        let source = Arc::clone(state.backend.source());
        state
            .task_monitor
            .instrument(async move {
                match source.data_sources().await {
                    Ok(result) => Json(result).into_response(),
                    Err(error) => error_response(error),
                }
            })
            .await
    }

    pub(crate) async fn data_source(
        State(state): State<Self>,
        Path(id): Path<i64>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId") else {
            return missing_parameter("serviceId");
        };
        if ServiceIdentity::parse(service_id).is_err() {
            return invalid_service_id();
        }
        match state
            .backend
            .source()
            .data_source(id, Some(service_id.as_str()))
            .await
        {
            Ok(Some(content)) => Json(DataSourceResult {
                result_code: MonitorStatService::RESULT_CODE_SUCCESS,
                content: Some(vec![content]),
            })
            .into_response(),
            Ok(None) => StatusCode::NOT_FOUND.into_response(),
            Err(error) => error_response(error),
        }
    }

    pub(crate) async fn sql_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Response {
        let source = Arc::clone(state.backend.source());
        state
            .task_monitor
            .instrument(async move {
                match source.sql_list(&query).await {
                    Ok(result) => Json(result).into_response(),
                    Err(error) => error_response(error),
                }
            })
            .await
    }

    pub(crate) async fn wall(
        State(state): State<Self>,
        Valid(Query(_query)): Valid<Query<StatQuery>>,
    ) -> Response {
        let source = Arc::clone(state.backend.source());
        state
            .task_monitor
            .instrument(async move {
                match source.wall().await {
                    Ok(result) => Json(result).into_response(),
                    Err(error) => error_response(error),
                }
            })
            .await
    }

    pub(crate) async fn api_description() -> Json<Value> {
        Json(json!({
            "ResultCode": MonitorStatService::RESULT_CODE_SUCCESS,
            "Content": {
                "endpoints": serde_json::from_str::<Value>(Self::endpoint_list())
                    .unwrap_or_else(|_| json!([])),
                "version": "standalone-v1",
            }
        }))
    }

    pub(crate) async fn web_uri_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Response {
        let Some(service) = state.backend.source().remote_service() else {
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

    pub(crate) async fn web_uri_detail(
        State(state): State<Self>,
        Path(uri): Path<String>,
    ) -> Json<Value> {
        let content = match state.backend.source().remote_service() {
            Some(service) => service.get_web_uri_stat_data(&uri),
            None => None,
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    pub(crate) async fn web_app_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        let content = match state.backend.source().remote_service() {
            Some(service) => service.get_web_app_stat_data_list(&query),
            None => Vec::new(),
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    pub(crate) async fn web_session_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        let content = match state.backend.source().remote_service() {
            Some(service) => service.get_web_session_stat_data_list(&query),
            None => Vec::new(),
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    pub(crate) async fn web_session_detail(
        State(state): State<Self>,
        Path(id): Path<String>,
    ) -> Json<Value> {
        let content = match state.backend.source().remote_service() {
            Some(service) => service.get_web_session_stat_data(&id),
            None => None,
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    pub(crate) async fn spring_list(
        State(state): State<Self>,
        Valid(Query(query)): Valid<Query<StatQuery>>,
    ) -> Json<Value> {
        let content = match state.backend.source().remote_service() {
            Some(service) => service.get_spring_stat_data_list(&query),
            None => Vec::new(),
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
    }

    pub(crate) async fn spring_detail(
        State(state): State<Self>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(class) = parameters.get("class") else {
            return missing_parameter("class");
        };
        let Some(method) = parameters.get("method") else {
            return missing_parameter("method");
        };
        let content = match state.backend.source().remote_service() {
            Some(service) => service.get_spring_method_stat_data(class, method),
            None => None,
        };
        Json(MonitorStatService::return_json_result(
            MonitorStatService::RESULT_CODE_SUCCESS,
            content,
        ))
        .into_response()
    }

    pub(crate) async fn sql_detail(
        State(state): State<Self>,
        Path(id): Path<i64>,
        Query(parameters): Query<HashMap<String, String>>,
    ) -> Response {
        let Some(service_id) = parameters.get("serviceId") else {
            return missing_parameter("serviceId");
        };
        if ServiceIdentity::parse(service_id).is_err() {
            return invalid_service_id();
        };
        let Some(datasource_id) = parameters
            .get("dataSourceId")
            .and_then(|value| value.parse::<i64>().ok())
        else {
            return missing_parameter("dataSourceId");
        };
        match state.backend.source().remote_service() {
            Some(service) => {
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
            None => match state
                .backend
                .source()
                .sql_stat(id, Some(service_id.as_str()), Some(datasource_id))
                .await
            {
                Ok(Some(stat)) => Json(stat).into_response(),
                Ok(None) => StatusCode::NOT_FOUND.into_response(),
                Err(error) => error_response(error),
            },
        }
    }

    pub(crate) async fn connection_info(
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

    pub(crate) async fn active(
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

    pub(crate) async fn legacy_dispatch(
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
        // Java 的控制台地址把标识符嵌在路径段中间（/datasource-{id}.json），
        // Axum 的路由匹配无法表达，先在控制台路由里解析。
        if let Some(response) = super::console_routes::dispatch_console_route(
            state.backend.source(),
            state.command_broker.as_ref(),
            service_url,
        )
        .await
        {
            return response;
        }
        match state.backend.source().remote_service() {
            Some(service) => match service.service(service_url).await {
                Ok(body) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/json;charset=utf-8")],
                    body,
                )
                    .into_response(),
                Err(error) => error_response(error),
            },
            None => Json(json!({
                "ResultCode": MonitorStatService::RESULT_CODE_ERROR,
                "Content": "Do not support this request, please contact with administrator.",
            }))
            .into_response(),
        }
    }

    pub(crate) async fn metrics(State(state): State<Self>) -> Response {
        let metrics = state.task_monitor.cumulative();
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
        write_repository_metrics(&mut body, state.backend.source().repository_view());
        if let Some(provider) = &state.metrics_appendix {
            body.push_str(&provider());
        }
        let (accepted, output_rejected_total, output_limit) = if state.backend.source().has_remote()
        {
            let limit = MetricsRepositoryConfig::default().max_metrics_output_bytes();
            let accepted = body.len() <= limit;
            let rejected = if accepted {
                state
                    .remote_metrics_output_rejected_total
                    .load(Ordering::Relaxed)
            } else {
                state
                    .remote_metrics_output_rejected_total
                    .fetch_add(1, Ordering::Relaxed)
                    + 1
            };
            (accepted, rejected, limit)
        } else {
            let repository = state.backend.source().repository_view();
            let accepted = repository.accept_metrics_output_size(body.len());
            let (_, _, rejected) = repository.limit_counters();
            (accepted, rejected, repository.max_metrics_output_bytes())
        };
        if !accepted {
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
                output_limit
            );
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
                rejected,
            )
                .into_response();
        }
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            body,
        )
            .into_response()
    }
}
