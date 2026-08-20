use super::{
    http_sql_prepared_statement::HttpSqlStatementExecutionError, HttpSqlDatabaseMetaData,
    HttpSqlPreparedStatement, HttpSqlProvider,
};
use base64::Engine;
use druid_core::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalDatabaseMetaData, PhysicalPreparedStatement, PhysicalResultSet, PreparedStatementKey,
    Row, RowSetResultSet, SqlWarning, StatementExecuteResult, StatementGeneratedKeys, Value,
};
use reqwest::{RequestBuilder, StatusCode, Url};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;

struct HttpSqlResult {
    rows: Vec<Row>,
    column_labels: Vec<String>,
    rows_affected: u64,
    last_insert_id: Option<i64>,
}

/// 单个 Druid holder 独占的 HTTP SQL 逻辑物理连接。
///
/// 该对象只持有独占的 HTTP transport client 和逻辑会话状态，不持有第三方数据库连接池。
pub struct HttpSqlConnectionAdapter {
    provider: HttpSqlProvider,
    endpoint: Url,
    properties: HashMap<String, String>,
    client: reqwest::Client,
    closed: bool,
    discarded: bool,
    product_version: Option<String>,
}

impl HttpSqlConnectionAdapter {
    pub(crate) fn new(
        provider: HttpSqlProvider,
        endpoint: impl Into<String>,
        properties: HashMap<String, String>,
        client: reqwest::Client,
    ) -> Result<Self, DruidError> {
        let endpoint = endpoint.into();
        let endpoint = Url::parse(&endpoint).map_err(|error| {
            DruidError::InvalidArgument(format!("invalid HTTP SQL URL: {error}"))
        })?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(DruidError::InvalidArgument(
                "HTTP SQL URL must use http or https".to_owned(),
            ));
        }
        if provider == HttpSqlProvider::CloudflareD1 && endpoint.scheme() != "https" {
            return Err(DruidError::InvalidArgument(
                "Cloudflare D1 endpoint must use https".to_owned(),
            ));
        }
        Ok(Self {
            provider,
            endpoint,
            properties,
            client,
            closed: false,
            discarded: false,
            product_version: None,
        })
    }

    fn ensure_open(&self) -> Result<(), DruidError> {
        if self.closed || self.discarded {
            Err(DruidError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn request_url(&self) -> Result<Url, DruidError> {
        let expected_suffix = match self.provider {
            HttpSqlProvider::Rqlite => "db/request",
            HttpSqlProvider::CloudflareD1 => "query",
        };
        if self
            .endpoint
            .path()
            .trim_end_matches('/')
            .ends_with(expected_suffix)
        {
            return Ok(self.endpoint.clone());
        }
        let mut endpoint = self.endpoint.clone();
        let path = endpoint.path().trim_end_matches('/');
        endpoint.set_path(&format!("{path}/{expected_suffix}"));
        Ok(endpoint)
    }

    fn authenticated(&self, request: RequestBuilder) -> RequestBuilder {
        let token = self
            .properties
            .get("token")
            .or_else(|| self.properties.get("api_token"));
        if let Some(token) = token {
            return request.bearer_auth(token);
        }
        match (self.properties.get("user"), self.properties.get("password")) {
            (Some(user), password) => request.basic_auth(user, password),
            _ => request,
        }
    }

    fn encode_parameter(&self, value: Value) -> JsonValue {
        match value {
            Value::Null => JsonValue::Null,
            Value::Bool(value) => JsonValue::Bool(value),
            Value::Int(value) => json!(value),
            Value::Float(value) => json!(value),
            Value::Decimal(value) => JsonValue::String(value.to_string()),
            Value::Date(value) => JsonValue::String(value.to_string()),
            Value::Time(value) => JsonValue::String(value.to_string()),
            Value::Timestamp(value) => JsonValue::String(value.to_string()),
            Value::String(value) => JsonValue::String(value),
            Value::Bytes(value) => match self.provider {
                HttpSqlProvider::Rqlite => json!({
                    "b64": base64::engine::general_purpose::STANDARD.encode(value)
                }),
                HttpSqlProvider::CloudflareD1 => {
                    JsonValue::Array(value.into_iter().map(|byte| json!(byte)).collect())
                }
            },
        }
    }

    fn decode_value(value: JsonValue) -> Result<Value, DruidError> {
        match value {
            JsonValue::Null => Ok(Value::Null),
            JsonValue::Bool(value) => Ok(Value::Bool(value)),
            JsonValue::Number(value) => value
                .as_i64()
                .map(Value::Int)
                .or_else(|| value.as_f64().map(Value::Float))
                .ok_or_else(|| DruidError::DriverError("invalid HTTP SQL number".to_owned())),
            JsonValue::String(value) => Ok(Value::String(value)),
            JsonValue::Array(values)
                if values
                    .iter()
                    .all(|value| value.as_u64().is_some_and(|byte| byte <= 255)) =>
            {
                Ok(Value::Bytes(
                    values
                        .into_iter()
                        .map(|value| value.as_u64().unwrap_or_default() as u8)
                        .collect(),
                ))
            }
            JsonValue::Object(mut value) if value.contains_key("b64") => {
                let encoded = value
                    .remove("b64")
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .ok_or_else(|| {
                        DruidError::DriverError("invalid rqlite b64 value".to_owned())
                    })?;
                base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map(Value::Bytes)
                    .map_err(|error| DruidError::DriverError(error.to_string()))
            }
            other => Err(DruidError::DriverError(format!(
                "unsupported HTTP SQL value {other}"
            ))),
        }
    }

    fn http_error(status: StatusCode, message: impl Into<String>) -> DruidError {
        let sql_state = if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            "28000"
        } else if matches!(
            status,
            StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT
        ) {
            "HYT00"
        } else if status.is_server_error() {
            "08006"
        } else {
            "HY000"
        };
        let exception = druid_core::core::SqlException::new(
            i32::from(status.as_u16()),
            Some(sql_state.to_owned()),
            Some(message.into()),
        )
        .with_class_name("druid.http_sql.HttpSqlException");
        DruidError::SqlException(Box::new(
            if status.is_server_error()
                || matches!(
                    status,
                    StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT
                )
            {
                exception.recoverable()
            } else {
                exception
            },
        ))
    }

    async fn request(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<HttpSqlResult, DruidError> {
        self.ensure_open()?;
        let parameters = params
            .into_iter()
            .map(|value| self.encode_parameter(value))
            .collect::<Vec<_>>();
        let body = match self.provider {
            HttpSqlProvider::Rqlite => {
                let mut statement = Vec::with_capacity(parameters.len() + 1);
                statement.push(JsonValue::String(sql.to_owned()));
                statement.extend(parameters);
                JsonValue::Array(vec![JsonValue::Array(statement)])
            }
            HttpSqlProvider::CloudflareD1 => json!({"sql": sql, "params": parameters}),
        };
        let request = self.authenticated(self.client.post(self.request_url()?).json(&body));
        let response = request.send().await.map_err(|error| {
            self.discarded = error.is_connect() || error.is_timeout();
            Self::http_error(StatusCode::SERVICE_UNAVAILABLE, error.to_string())
        })?;
        let status = response.status();
        let payload: JsonValue = response.json().await.map_err(|error| {
            self.discarded = true;
            Self::http_error(status, format!("invalid HTTP SQL response: {error}"))
        })?;
        if !status.is_success() {
            if status.is_server_error()
                || matches!(
                    status,
                    StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT
                )
            {
                self.discarded = true;
            }
            return Err(Self::http_error(status, payload.to_string()));
        }
        match self.provider {
            HttpSqlProvider::Rqlite => Self::decode_rqlite(payload),
            HttpSqlProvider::CloudflareD1 => Self::decode_d1(payload),
        }
    }

    fn decode_rqlite(payload: JsonValue) -> Result<HttpSqlResult, DruidError> {
        let result = payload
            .get("results")
            .and_then(JsonValue::as_array)
            .and_then(|results| results.first())
            .ok_or_else(|| Self::http_error(StatusCode::BAD_GATEWAY, payload.to_string()))?;
        if let Some(message) = result.get("error").and_then(JsonValue::as_str) {
            return Err(Self::http_error(StatusCode::BAD_REQUEST, message));
        }
        let labels = result
            .get("columns")
            .and_then(JsonValue::as_array)
            .map(|columns| {
                columns
                    .iter()
                    .filter_map(JsonValue::as_str)
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let rows = result
            .get("values")
            .and_then(JsonValue::as_array)
            .map(|rows| {
                rows.iter()
                    .map(|row| {
                        row.as_array()
                            .ok_or_else(|| {
                                DruidError::DriverError("invalid rqlite row".to_owned())
                            })?
                            .iter()
                            .cloned()
                            .map(Self::decode_value)
                            .collect::<Result<Vec<_>, _>>()
                            .map(Row::new)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(HttpSqlResult {
            rows,
            column_labels: labels,
            rows_affected: result
                .get("rows_affected")
                .and_then(JsonValue::as_u64)
                .unwrap_or_default(),
            last_insert_id: result.get("last_insert_id").and_then(JsonValue::as_i64),
        })
    }

    fn decode_d1(payload: JsonValue) -> Result<HttpSqlResult, DruidError> {
        if payload.get("success").and_then(JsonValue::as_bool) != Some(true) {
            let error = payload
                .get("errors")
                .and_then(JsonValue::as_array)
                .and_then(|errors| errors.first())
                .cloned()
                .unwrap_or(payload);
            return Err(Self::http_error(StatusCode::BAD_REQUEST, error.to_string()));
        }
        let result = payload
            .get("result")
            .and_then(JsonValue::as_array)
            .and_then(|results| results.first())
            .ok_or_else(|| Self::http_error(StatusCode::BAD_GATEWAY, "missing D1 result"))?;
        let objects = result
            .get("results")
            .and_then(JsonValue::as_array)
            .cloned()
            .unwrap_or_default();
        let labels = objects
            .first()
            .and_then(JsonValue::as_object)
            .map(|object| object.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        let rows = objects
            .into_iter()
            .map(|row| {
                let mut object = row.as_object().cloned().ok_or_else(|| {
                    DruidError::DriverError("invalid Cloudflare D1 row".to_owned())
                })?;
                labels
                    .iter()
                    .map(|label| {
                        Self::decode_value(object.remove(label).unwrap_or(JsonValue::Null))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(Row::new)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let metadata = result.get("meta").and_then(JsonValue::as_object);
        Ok(HttpSqlResult {
            rows,
            column_labels: labels,
            rows_affected: metadata
                .and_then(|meta| meta.get("changes"))
                .and_then(JsonValue::as_u64)
                .unwrap_or_default(),
            last_insert_id: metadata
                .and_then(|meta| meta.get("last_row_id"))
                .and_then(JsonValue::as_i64),
        })
    }

    fn prepared_statement(
        statement: &dyn PhysicalPreparedStatement,
    ) -> Result<&HttpSqlPreparedStatement, DruidError> {
        statement
            .as_any()
            .downcast_ref::<HttpSqlPreparedStatement>()
            .ok_or_else(|| {
                DruidError::DriverError(
                    "prepared statement was not created by HttpSqlConnectionAdapter".to_owned(),
                )
            })
    }

    fn finish_controlled<T>(
        &mut self,
        result: Result<T, HttpSqlStatementExecutionError>,
    ) -> Result<T, DruidError> {
        match result {
            Ok(value) => Ok(value),
            Err(HttpSqlStatementExecutionError::Driver(error)) => Err(error),
            Err(HttpSqlStatementExecutionError::TimedOut) => {
                self.discarded = true;
                Err(DruidError::SqlException(Box::new(
                    druid_core::core::SqlException::new(
                        0,
                        Some("HYT00".to_owned()),
                        Some("HTTP SQL request timed out".to_owned()),
                    )
                    .with_class_name("java.sql.SQLTimeoutException"),
                )))
            }
            Err(HttpSqlStatementExecutionError::Cancelled) => {
                self.discarded = true;
                Err(DruidError::SqlException(Box::new(
                    druid_core::core::SqlException::new(
                        0,
                        Some("HY008".to_owned()),
                        Some("HTTP SQL request was cancelled".to_owned()),
                    )
                    .with_class_name("java.sql.SQLException"),
                )))
            }
        }
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for HttpSqlConnectionAdapter {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        let result = self.request(sql, params).await?;
        Ok(ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: Some(u64::try_from(result.rows.len()).unwrap_or(u64::MAX)),
        })
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        _generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let result = self.request(sql, params).await?;
        if result.column_labels.is_empty() {
            Ok(vec![StatementExecuteResult::Update(ExecResult {
                rows_affected: result.rows_affected,
                last_insert_id: result.last_insert_id,
                row_count: None,
            })])
        } else {
            Ok(vec![StatementExecuteResult::ResultSet(result.rows)])
        }
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.request(sql, params).await.map(|result| result.rows)
    }

    async fn fetch_result_set(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let result = self.request(sql, params).await?;
        Ok(Arc::new(RowSetResultSet::with_column_labels(
            result.rows,
            result.column_labels,
        )))
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.ensure_open()?;
        Ok(Arc::new(HttpSqlPreparedStatement::new(key.sql())))
    }

    async fn exec_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        let statement = Self::prepared_statement(statement)?;
        let result = statement
            .execute_with_controls(self.request(statement.sql(), params))
            .await;
        let result = self.finish_controlled(result)?;
        Ok(ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: Some(u64::try_from(result.rows.len()).unwrap_or(u64::MAX)),
        })
    }

    async fn fetch_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let statement = Self::prepared_statement(statement)?;
        let result = statement
            .execute_with_controls(self.request(statement.sql(), params))
            .await;
        self.finish_controlled(result).map(|result| result.rows)
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        let result = self.request("SELECT 1", Vec::new()).await?;
        if result.rows.is_empty() {
            return Err(DruidError::ValidationFailed(
                "HTTP SQL validation returned no rows".to_owned(),
            ));
        }
        Ok(())
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "http_sql_transactions",
        })
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "http_sql_transactions",
        })
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "http_sql_transactions",
        })
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        PhysicalConnectionCapabilities {
            transactions: false,
            savepoints: false,
            auto_commit: false,
            read_only: false,
            transaction_isolation: false,
            holdability: false,
            clear_warnings: true,
            catalog: false,
            schema: false,
        }
    }

    fn database_meta_data(&mut self) -> Result<Box<dyn PhysicalDatabaseMetaData + '_>, DruidError> {
        self.ensure_open()?;
        Ok(Box::new(HttpSqlDatabaseMetaData::new(
            self.provider,
            self.endpoint.to_string(),
            self.product_version.clone(),
        )))
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        self.ensure_open()?;
        Ok(None)
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.ensure_open()
    }

    fn mark_discarded(&mut self) {
        self.discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.discarded
    }

    fn driver_name(&self) -> &str {
        self.provider.as_str()
    }
}
