use super::HttpSqlProvider;
use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{
    owned_executor, AdapterExecutionCore, AdapterUpdateResult, StandardExecutor,
};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use base64::Engine;
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::{Row, Value};
use reqwest::{StatusCode, Url};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;

struct HttpSqlResult {
    rows: Vec<Row>,
    column_labels: Vec<String>,
    rows_affected: u64,
    last_insert_id: Option<i64>,
}

#[async_trait::async_trait]
pub(super) trait HttpSqlTransport: Send + Sync {
    async fn send(
        &self,
        endpoint: Url,
        properties: &HashMap<String, String>,
        body: JsonValue,
    ) -> Result<(StatusCode, JsonValue), String>;
}

pub(super) struct ReqwestHttpSqlTransport {
    client: reqwest::Client,
}

impl ReqwestHttpSqlTransport {
    pub(super) fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl HttpSqlTransport for ReqwestHttpSqlTransport {
    async fn send(
        &self,
        endpoint: Url,
        properties: &HashMap<String, String>,
        body: JsonValue,
    ) -> Result<(StatusCode, JsonValue), String> {
        let mut request = self.client.post(endpoint).json(&body);
        if let Some(token) = properties
            .get("token")
            .or_else(|| properties.get("api_token"))
        {
            request = request.bearer_auth(token);
        } else if let Some(user) = properties.get("user") {
            request = request.basic_auth(user, properties.get("password"));
        }
        let response = request.send().await.map_err(|error| error.to_string())?;
        let status = response.status();
        let payload = response.json().await.map_err(|error| error.to_string())?;
        Ok((status, payload))
    }
}

/// 单个 Druid holder 独占的 HTTP SQL 逻辑物理连接。
///
/// 该对象只持有独占的 HTTP transport client 和逻辑会话状态，不持有第三方数据库连接池。
pub struct HttpSqlConnectionAdapter {
    provider: HttpSqlProvider,
    endpoint: Url,
    properties: HashMap<String, String>,
    transport: Arc<dyn HttpSqlTransport>,
    closed: bool,
    discarded: bool,
    product_version: Option<String>,
    standard_executor: Option<Arc<dyn StandardExecutor>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl HttpSqlConnectionAdapter {
    pub(super) fn new(
        provider: HttpSqlProvider,
        endpoint: impl Into<String>,
        properties: HashMap<String, String>,
        transport: Arc<dyn HttpSqlTransport>,
    ) -> Result<Self, RdbcError> {
        let endpoint = endpoint.into();
        let endpoint = Url::parse(&endpoint).map_err(|error| {
            RdbcError::InvalidArgument(format!("invalid HTTP SQL URL: {error}"))
        })?;
        if !matches!(endpoint.scheme(), "http" | "https") {
            return Err(RdbcError::InvalidArgument(
                "HTTP SQL URL must use http or https".to_owned(),
            ));
        }
        if provider == HttpSqlProvider::CloudflareD1 && endpoint.scheme() != "https" {
            return Err(RdbcError::InvalidArgument(
                "Cloudflare D1 endpoint must use https".to_owned(),
            ));
        }
        Ok(Self {
            provider,
            endpoint,
            properties,
            transport,
            closed: false,
            discarded: false,
            product_version: None,
            standard_executor: None,
            runtime_trace: None,
        })
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    fn standard_executor_core(&mut self) -> Result<Arc<dyn StandardExecutor>, RdbcError> {
        if let Some(executor) = &self.standard_executor {
            return Ok(Arc::clone(executor));
        }
        self.ensure_open()?;
        let runtime_trace = self.runtime_trace.take();
        let shell = Self {
            provider: self.provider,
            endpoint: self.endpoint.clone(),
            properties: self.properties.clone(),
            transport: Arc::clone(&self.transport),
            closed: false,
            discarded: self.discarded,
            product_version: self.product_version.clone(),
            standard_executor: None,
            runtime_trace: None,
        };
        let owned = std::mem::replace(self, shell);
        let executor = owned_executor(owned, runtime_trace);
        self.standard_executor = Some(Arc::clone(&executor));
        Ok(executor)
    }

    async fn execute_update_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        let result = self.request(sql, params).await?;
        Ok(AdapterUpdateResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: Some(u64::try_from(result.rows.len()).unwrap_or(u64::MAX)),
        })
    }

    #[allow(
        clippy::unused_async,
        reason = "uniform async close core across adapter families"
    )]
    async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed || self.discarded {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn request_url(&self) -> Result<Url, RdbcError> {
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

    fn decode_value(value: JsonValue) -> Result<Value, RdbcError> {
        match value {
            JsonValue::Null => Ok(Value::Null),
            JsonValue::Bool(value) => Ok(Value::Bool(value)),
            JsonValue::Number(value) => value
                .as_i64()
                .map(Value::Int)
                .or_else(|| value.as_f64().map(Value::Float))
                .ok_or_else(|| RdbcError::DriverError("invalid HTTP SQL number".to_owned())),
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
                    .ok_or_else(|| RdbcError::DriverError("invalid rqlite b64 value".to_owned()))?;
                base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map(Value::Bytes)
                    .map_err(|error| RdbcError::DriverError(error.to_string()))
            }
            other => Err(RdbcError::DriverError(format!(
                "unsupported HTTP SQL value {other}"
            ))),
        }
    }

    fn http_error(status: StatusCode, message: impl Into<String>) -> RdbcError {
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
        let exception = druid_rdbc::error::SqlException::new(
            i32::from(status.as_u16()),
            Some(sql_state.to_owned()),
            Some(message.into()),
        )
        .with_class_name("druid.http_sql.HttpSqlException");
        RdbcError::SqlException(Box::new(
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

    async fn request(&mut self, sql: &str, params: Vec<Value>) -> Result<HttpSqlResult, RdbcError> {
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
        let (status, payload) = self
            .transport
            .send(self.request_url()?, &self.properties, body)
            .await
            .map_err(|error| {
                self.discarded = true;
                Self::http_error(StatusCode::SERVICE_UNAVAILABLE, error)
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

    fn decode_rqlite(payload: JsonValue) -> Result<HttpSqlResult, RdbcError> {
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
                            .ok_or_else(|| RdbcError::DriverError("invalid rqlite row".to_owned()))?
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

    fn decode_d1(payload: JsonValue) -> Result<HttpSqlResult, RdbcError> {
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
                    RdbcError::DriverError("invalid Cloudflare D1 row".to_owned())
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
}

#[async_trait::async_trait]
impl AdapterExecutionCore for HttpSqlConnectionAdapter {
    async fn standard_execute_update_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<i64, RdbcError> {
        let result = self.execute_update_core(sql, values).await?;
        i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".to_owned())
        })
    }

    async fn standard_execute_query_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError> {
        self.request(sql, values)
            .await
            .map(|result| StandardQueryResult::with_labels(result.rows, result.column_labels))
    }

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.close_connection_core().await
    }
}

druid_rdbc::impl_standard_wrapper!(HttpSqlConnectionAdapter => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for HttpSqlConnectionAdapter {
    async fn create_statement(
        &mut self,
        _options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Ok(Box::new(StandardAdapterStatement::new(
            self.standard_executor_core()?,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        _options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let parameter_count = executor.parameter_count(sql).await?;
        Ok(Box::new(StandardAdapterPreparedStatement::new(
            sql.to_owned(),
            executor,
            parameter_count,
        )))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        let product_name = match self.provider {
            HttpSqlProvider::Rqlite => "rqlite",
            HttpSqlProvider::CloudflareD1 => "Cloudflare D1",
        };
        Ok(Box::new(StandardAdapterDatabaseMetaData::new(
            self.standard_executor_core()?,
            Some(product_name.to_owned()),
            "http_sql_database_metadata_product_name",
        )))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if let Some(executor) = &self.standard_executor {
            executor.close().await
        } else {
            self.close_connection_core().await
        }
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self
            .standard_executor
            .as_ref()
            .map_or(self.closed, |executor| executor.is_closed()))
    }

    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        self.request("SELECT 1", Vec::new()).await.map(|_| true)
    }
}
