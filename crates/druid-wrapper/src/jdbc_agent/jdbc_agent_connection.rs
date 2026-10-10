use super::{AgentValue, JdbcAgentClient, JdbcAgentDatabaseMetaData, JdbcAgentOptions};
use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{
    owned_executor, AdapterExecutionCore, AdapterUpdateResult, StandardExecutor,
};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::{Row, Value};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;

enum JdbcAgentExecutionResult {
    Query,
    Update(AdapterUpdateResult),
}

#[async_trait::async_trait]
pub(super) trait JdbcAgentTransport: Send {
    async fn request(
        &mut self,
        operation: &str,
        payload: JsonValue,
    ) -> Result<JsonValue, RdbcError>;
    async fn close(&mut self) -> Result<(), RdbcError>;
    fn is_unusable(&self) -> bool;
}

#[async_trait::async_trait]
impl JdbcAgentTransport for JdbcAgentClient {
    async fn request(
        &mut self,
        operation: &str,
        payload: JsonValue,
    ) -> Result<JsonValue, RdbcError> {
        JdbcAgentClient::request(self, operation, payload).await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        JdbcAgentClient::close(self).await
    }

    fn is_unusable(&self) -> bool {
        JdbcAgentClient::is_unusable(self)
    }
}

#[async_trait::async_trait]
pub(super) trait JdbcAgentConnector: Send + Sync {
    async fn connect(
        &self,
        options: JdbcAgentOptions,
        payload: JsonValue,
    ) -> Result<(Box<dyn JdbcAgentTransport>, JsonValue), RdbcError>;
}

pub(super) struct NativeJdbcAgentConnector;

#[async_trait::async_trait]
impl JdbcAgentConnector for NativeJdbcAgentConnector {
    async fn connect(
        &self,
        options: JdbcAgentOptions,
        payload: JsonValue,
    ) -> Result<(Box<dyn JdbcAgentTransport>, JsonValue), RdbcError> {
        let (client, session) = JdbcAgentClient::connect(options, payload).await?;
        Ok((Box::new(client), session))
    }
}

/// 由 Druid 独占并通过 JDBC Agent 操作的未池化物理连接。
#[allow(clippy::struct_excessive_bools)]
pub struct JdbcAgentConnection {
    client: Option<Box<dyn JdbcAgentTransport>>,
    capabilities: druid_rdbc::extensions::ConnectionCapabilities,
    auto_commit: bool,
    read_only: bool,
    transaction_isolation: u8,
    catalog: Option<String>,
    schema: Option<String>,
    savepoints: HashMap<u64, String>,
    next_savepoint_id: u64,
    closed: bool,
    discarded: bool,
    metadata: JdbcAgentDatabaseMetaData,
    driver_name: String,
    standard_executor: Option<Arc<dyn StandardExecutor>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl JdbcAgentConnection {
    /// 启动 Agent，并用显式 JDBC URL、属性和验证 SQL 建立一个物理连接。
    pub async fn connect(
        url: &str,
        validation_query: Option<&str>,
        properties: HashMap<String, String>,
        options: JdbcAgentOptions,
    ) -> Result<Self, RdbcError> {
        Self::connect_with_connector(
            url,
            validation_query,
            properties,
            options,
            Arc::new(NativeJdbcAgentConnector),
        )
        .await
    }

    pub(super) async fn connect_with_connector(
        url: &str,
        validation_query: Option<&str>,
        properties: HashMap<String, String>,
        options: JdbcAgentOptions,
        connector: Arc<dyn JdbcAgentConnector>,
    ) -> Result<Self, RdbcError> {
        let (client, session) = connector
            .connect(
                options,
                json!({
                    "url": url,
                    "properties": properties,
                    "validationQuery": validation_query,
                }),
            )
            .await?;
        let capability = |name: &str| {
            session
                .pointer(&format!("/capabilities/{name}"))
                .and_then(JsonValue::as_bool)
                .unwrap_or(false)
        };
        let supports_transactions = capability("transactions");
        let driver_name = session
            .get("driverName")
            .and_then(JsonValue::as_str)
            .unwrap_or("jdbc-agent")
            .to_owned();
        let metadata = JdbcAgentDatabaseMetaData::new(
            session
                .get("databaseProductName")
                .and_then(JsonValue::as_str)
                .map(str::to_owned),
        );
        Ok(Self {
            client: Some(client),
            capabilities: druid_rdbc::extensions::ConnectionCapabilities {
                transactions: supports_transactions,
                savepoints: capability("savepoints"),
                catalogs: capability("catalog"),
                schemas: capability("schema"),
                callable_statements: true,
            },
            auto_commit: session
                .get("autoCommit")
                .and_then(JsonValue::as_bool)
                .unwrap_or(true),
            read_only: session
                .get("readOnly")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false),
            transaction_isolation: session
                .get("transactionIsolation")
                .and_then(JsonValue::as_u64)
                .and_then(|value| u8::try_from(value).ok())
                .unwrap_or(2),
            catalog: session
                .get("catalog")
                .and_then(JsonValue::as_str)
                .map(ToOwned::to_owned),
            schema: session
                .get("schema")
                .and_then(JsonValue::as_str)
                .map(ToOwned::to_owned),
            savepoints: HashMap::new(),
            next_savepoint_id: 1,
            closed: false,
            discarded: false,
            metadata,
            driver_name,
            standard_executor: None,
            runtime_trace: None,
        })
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    fn client_mut_core(&mut self) -> Result<&mut (dyn JdbcAgentTransport + '_), RdbcError> {
        match self.client.as_deref_mut() {
            Some(client) => Ok(client),
            None => Err(RdbcError::ConnectionDiscarded),
        }
    }

    fn standard_executor_core(&mut self) -> Result<Arc<dyn StandardExecutor>, RdbcError> {
        if let Some(executor) = &self.standard_executor {
            return Ok(Arc::clone(executor));
        }
        self.ensure_open()?;
        let runtime_trace = self.runtime_trace.take();
        let shell = Self {
            client: None,
            capabilities: self.capabilities,
            auto_commit: self.auto_commit,
            read_only: self.read_only,
            transaction_isolation: self.transaction_isolation,
            catalog: self.catalog.clone(),
            schema: self.schema.clone(),
            savepoints: HashMap::new(),
            next_savepoint_id: self.next_savepoint_id,
            closed: false,
            discarded: self.discarded,
            metadata: self.metadata.clone(),
            driver_name: self.driver_name.clone(),
            standard_executor: None,
            runtime_trace: None,
        };
        let owned = std::mem::replace(self, shell);
        let executor = owned_executor(owned, runtime_trace);
        self.standard_executor = Some(Arc::clone(&executor));
        Ok(executor)
    }

    async fn create_savepoint(
        &mut self,
        name: Option<&str>,
    ) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        let local_id = self.next_savepoint_id;
        let next_id = local_id.checked_add(1).ok_or_else(|| {
            RdbcError::InvalidArgument("JDBC Agent savepoint id exhausted".to_owned())
        })?;
        let response = self
            .standard_executor_core()?
            .connection_control("set_savepoint", json!({"name": name}))
            .await?;
        let remote_id = response
            .get("savepointId")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| Self::protocol_error("set_savepoint returned no savepointId"))?;
        self.savepoints.insert(local_id, remote_id.to_owned());
        self.next_savepoint_id = next_id;
        Ok(druid_rdbc::types::Savepoint {
            id: local_id,
            name: name.map(str::to_owned),
        })
    }

    async fn execute_update_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        self.ensure_open()?;
        let results = self
            .execute_request(
                sql,
                params,
                druid_rdbc::statement::GeneratedKeys::ReturnGeneratedKeys,
            )
            .await?;
        match results.into_iter().next() {
            Some(JdbcAgentExecutionResult::Update(result)) => Ok(result),
            Some(JdbcAgentExecutionResult::Query) => Err(Self::protocol_error(
                "exec received a result set instead of an update count",
            )),
            None => Err(Self::protocol_error("execute returned no result")),
        }
    }

    async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
        if self.closed {
            return Ok(());
        }
        let result = match self.client.as_mut() {
            Some(client) => client.close().await,
            None => Ok(()),
        };
        if result.is_ok() {
            self.client.take();
            self.closed = true;
        }
        result
    }

    /// 终止当前隔离 Agent，用于认证其崩溃传播与连接丢弃合同。
    #[doc(hidden)]
    pub async fn diagnostic_crash_agent(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("diagnostic_crash", JsonValue::Null)
            .await
            .map(|_| ())
    }

    /// 令当前隔离 Agent 输出非法协议帧，用于认证 fail-closed 行为。
    #[doc(hidden)]
    pub async fn diagnostic_protocol_failure(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("diagnostic_protocol_failure", JsonValue::Null)
            .await
            .map(|_| ())
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed
            || self.discarded
            || self
                .standard_executor
                .as_ref()
                .is_some_and(|executor| executor.is_closed())
            || (self.standard_executor.is_none()
                && self
                    .client
                    .as_deref()
                    .is_none_or(JdbcAgentTransport::is_unusable))
        {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn parameters(params: Vec<Value>) -> Result<Vec<AgentValue>, RdbcError> {
        params.into_iter().map(AgentValue::from_druid).collect()
    }

    fn rows(payload: &JsonValue) -> Result<Vec<Row>, RdbcError> {
        let rows = payload
            .get("rows")
            .cloned()
            .ok_or_else(|| Self::protocol_error("fetch response does not contain rows"))?;
        let rows: Vec<Vec<AgentValue>> = serde_json::from_value(rows)
            .map_err(|error| Self::protocol_error(error.to_string()))?;
        rows.into_iter()
            .map(|values| {
                values
                    .into_iter()
                    .map(AgentValue::into_druid)
                    .collect::<Result<Vec<_>, _>>()
                    .map(Row::new)
            })
            .collect()
    }

    fn column_labels(payload: &JsonValue) -> Result<Vec<String>, RdbcError> {
        payload
            .get("columns")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| Self::protocol_error("fetch response does not contain columns"))?
            .iter()
            .map(|column| {
                column
                    .get("label")
                    .and_then(JsonValue::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| Self::protocol_error("result column does not contain label"))
            })
            .collect()
    }

    async fn collect_query_payload(
        &mut self,
        mut payload: JsonValue,
    ) -> Result<Vec<Row>, RdbcError> {
        let mut result = Vec::new();
        loop {
            let has_more = payload
                .get("hasMore")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false);
            let cursor_id = payload
                .get("cursorId")
                .and_then(JsonValue::as_str)
                .map(ToOwned::to_owned);
            match Self::rows(&payload) {
                Ok(mut rows) => result.append(&mut rows),
                Err(error) => {
                    if let Some(cursor_id) = cursor_id {
                        let _ = self
                            .client_mut_core()?
                            .request("close_cursor", json!({"cursorId": cursor_id}))
                            .await;
                    }
                    return Err(error);
                }
            }
            if !has_more {
                return Ok(result);
            }
            let cursor_id = cursor_id.ok_or_else(|| {
                Self::protocol_error("paged result hasMore=true but cursorId is absent")
            })?;
            payload = match self
                .client_mut_core()?
                .request(
                    "fetch_page",
                    json!({
                        "cursorId": cursor_id,
                        "pageSize": 500,
                        "maxResponseBytes": 8 * 1024 * 1024,
                    }),
                )
                .await
            {
                Ok(payload) => payload,
                Err(error) => {
                    let _ = self
                        .client_mut_core()?
                        .request("close_cursor", json!({"cursorId": cursor_id}))
                        .await;
                    return Err(error);
                }
            };
        }
    }

    fn exec_result(payload: &JsonValue) -> Result<AdapterUpdateResult, RdbcError> {
        let rows_affected = payload
            .get("rowsAffected")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| Self::protocol_error("exec response does not contain rowsAffected"))?;
        let last_insert_id = match payload.get("lastInsertId") {
            None | Some(JsonValue::Null) => None,
            Some(value) => Some(
                value
                    .as_i64()
                    .ok_or_else(|| Self::protocol_error("lastInsertId is not a signed integer"))?,
            ),
        };
        Ok(AdapterUpdateResult {
            rows_affected,
            last_insert_id,
            row_count: None,
        })
    }

    fn generated_keys(generated_keys: druid_rdbc::statement::GeneratedKeys) -> JsonValue {
        match generated_keys {
            druid_rdbc::statement::GeneratedKeys::ReturnGeneratedKeys => {
                json!({"mode": "auto", "value": 1})
            }
            druid_rdbc::statement::GeneratedKeys::ColumnIndexes(value) => {
                let value = value
                    .into_iter()
                    .map(druid_rdbc::result_set::ColumnIndex::get)
                    .collect::<Vec<_>>();
                json!({"mode": "column_indexes", "value": value})
            }
            druid_rdbc::statement::GeneratedKeys::ColumnNames(value) => {
                json!({"mode": "column_names", "value": value})
            }
            _ => json!({"mode": "none"}),
        }
    }

    fn protocol_error(message: impl Into<String>) -> RdbcError {
        RdbcError::SqlException(Box::new(
            druid_rdbc::error::SqlException::new(
                0,
                Some("08006".to_owned()),
                Some(format!("invalid JDBC Agent response: {}", message.into())),
            )
            .with_class_name("druid.jdbc_agent.ProtocolException")
            .recoverable(),
        ))
    }

    async fn execute_request(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Vec<JdbcAgentExecutionResult>, RdbcError> {
        let payload = self
            .client_mut_core()?
            .request(
                "execute",
                json!({
                    "sql": sql,
                    "params": Self::parameters(params)?,
                    "generatedKeys": Self::generated_keys(generated_keys),
                }),
            )
            .await?;
        match payload.get("kind").and_then(JsonValue::as_str) {
            Some("result_set") => {
                let _ = self.collect_query_payload(payload).await?;
                Ok(vec![JdbcAgentExecutionResult::Query])
            }
            Some("update") => Ok(vec![JdbcAgentExecutionResult::Update(Self::exec_result(
                &payload,
            )?)]),
            Some(kind) => Err(Self::protocol_error(format!(
                "unsupported execute result kind '{kind}'"
            ))),
            None => Err(Self::protocol_error(
                "execute response does not contain kind",
            )),
        }
    }
}

impl std::fmt::Debug for JdbcAgentConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentConnection")
            .field("auto_commit", &self.auto_commit)
            .field("read_only", &self.read_only)
            .field("transaction_isolation", &self.transaction_isolation)
            .field("closed", &self.closed)
            .field("discarded", &self.discarded)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl AdapterExecutionCore for JdbcAgentConnection {
    async fn standard_connection_control_core(
        &mut self,
        operation: &str,
        payload: JsonValue,
    ) -> Result<JsonValue, RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?.request(operation, payload).await
    }

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
        self.ensure_open()?;
        let payload = self
            .client_mut_core()?
            .request(
                "execute_query",
                json!({"sql": sql, "params": Self::parameters(values)?}),
            )
            .await?;
        let labels = Self::column_labels(&payload)?;
        self.collect_query_payload(payload)
            .await
            .map(|rows| StandardQueryResult::with_labels(rows, labels))
    }

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.close_connection_core().await
    }
}

druid_rdbc::impl_standard_wrapper!(JdbcAgentConnection => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for JdbcAgentConnection {
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(self.auto_commit)
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("set_auto_commit", json!({"value": enabled}))
            .await?;
        self.auto_commit = enabled;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("commit", JsonValue::Null)
            .await?;
        self.savepoints.clear();
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("rollback", JsonValue::Null)
            .await?;
        self.savepoints.clear();
        Ok(())
    }

    async fn set_savepoint(&mut self) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        self.create_savepoint(None).await
    }

    async fn set_savepoint_named(
        &mut self,
        name: &str,
    ) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        self.create_savepoint(Some(name)).await
    }

    async fn rollback_to(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let remote_id = self
            .savepoints
            .get(&savepoint.id)
            .ok_or_else(|| RdbcError::InvalidArgument("unknown JDBC Agent savepoint".to_owned()))?
            .clone();
        self.standard_executor_core()?
            .connection_control("rollback_to_savepoint", json!({"savepointId": remote_id}))
            .await?;
        Ok(())
    }

    async fn release_savepoint(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let remote_id = self
            .savepoints
            .get(&savepoint.id)
            .ok_or_else(|| RdbcError::InvalidArgument("unknown JDBC Agent savepoint".to_owned()))?
            .clone();
        self.standard_executor_core()?
            .connection_control("release_savepoint", json!({"savepointId": remote_id}))
            .await?;
        self.savepoints.remove(&savepoint.id);
        Ok(())
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        Ok(self.catalog.clone())
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("set_catalog", json!({"value": catalog}))
            .await?;
        self.catalog = catalog.map(str::to_owned);
        Ok(())
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        Ok(self.schema.clone())
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        self.standard_executor_core()?
            .connection_control("set_schema", json!({"value": schema}))
            .await?;
        self.schema = schema.map(str::to_owned);
        Ok(())
    }

    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<druid_rdbc::statement::ExecResult, RdbcError> {
        let result = self
            .standard_executor_core()?
            .execute_update_result(sql, params)
            .await?;
        Ok(druid_rdbc::statement::ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: result.row_count,
        })
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        self.standard_executor_core()?
            .execute_query(sql, params)
            .await
            .map(|result| result.rows)
    }

    async fn is_valid(&mut self, timeout_seconds: u32) -> Result<bool, RdbcError> {
        let executor = self.standard_executor_core()?;
        let query = executor.execute_query("SELECT 1", Vec::new());
        if timeout_seconds == 0 {
            query.await?;
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(u64::from(timeout_seconds)),
                query,
            )
            .await
            .map_err(|_| {
                RdbcError::DriverError("JDBC Agent connection validation timed out".to_owned())
            })??;
        }
        Ok(true)
    }

    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Ok(Box::new(StandardAdapterStatement::new(
            self.standard_executor_core()?,
            options,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let parameter_count = executor.parameter_count(sql).await?;
        Ok(Box::new(StandardAdapterPreparedStatement::new(
            sql.to_owned(),
            executor,
            parameter_count,
            options,
        )))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        let mut snapshot = self.metadata.clone();
        let product_name =
            druid_rdbc::metadata::DatabaseMetaData::database_product_name(&mut snapshot)
                .await
                .ok();
        Ok(Box::new(
            StandardAdapterDatabaseMetaData::new(
                self.standard_executor_core()?,
                product_name,
                "jdbc_agent_database_metadata_product_name_absent",
            )
            .with_driver_name(self.driver_name.clone()),
        ))
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

    fn capabilities(&self) -> druid_rdbc::extensions::ConnectionCapabilities {
        self.capabilities
    }
}
