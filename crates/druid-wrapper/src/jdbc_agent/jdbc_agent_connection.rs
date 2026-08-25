use super::{
    AgentValue, JdbcAgentClient, JdbcAgentDatabaseMetaData, JdbcAgentOptions,
    JdbcAgentPreparedStatement,
};
use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{owned_executor, AdapterExecutionCore, StandardExecutor};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use druid_rdbc::error::RdbcError;
use druid_rdbc::spi::{
    ExecResult, PhysicalConnection, PhysicalConnectionCapabilities, PhysicalDatabaseMetaData,
    PhysicalPreparedStatement, PhysicalResultSet, PreparedStatementKey, RowSetResultSet,
    StatementExecuteResult, StatementGeneratedKeys,
};
use druid_rdbc::types::{Row, Savepoint, Value};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait::async_trait]
pub(super) trait JdbcAgentTransport: Send {
    async fn request(
        &mut self,
        operation: &str,
        payload: JsonValue,
    ) -> Result<JsonValue, RdbcError>;
    async fn close(&mut self) -> Result<(), RdbcError>;
    fn is_unusable(&self) -> bool;
    fn statement_context(&self) -> Option<(super::AgentRequestHandle, String)>;
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

    fn statement_context(&self) -> Option<(super::AgentRequestHandle, String)> {
        Some(JdbcAgentClient::statement_context(self))
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
    capabilities: PhysicalConnectionCapabilities,
    auto_commit: bool,
    read_only: bool,
    transaction_isolation: u8,
    catalog: Option<String>,
    schema: Option<String>,
    closed: bool,
    discarded: bool,
    savepoint_sequence: u64,
    savepoint_ids: HashMap<u64, String>,
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
            url,
            Some(driver_name.clone()),
            session
                .get("driverVersion")
                .and_then(JsonValue::as_str)
                .map(str::to_owned),
            session
                .get("databaseProductName")
                .and_then(JsonValue::as_str)
                .map(str::to_owned),
            session
                .get("databaseProductVersion")
                .and_then(JsonValue::as_str)
                .map(str::to_owned),
            supports_transactions,
        );
        Ok(Self {
            client: Some(client),
            capabilities: PhysicalConnectionCapabilities {
                transactions: supports_transactions,
                savepoints: capability("savepoints"),
                auto_commit: capability("autoCommit"),
                read_only: capability("readOnly"),
                transaction_isolation: capability("transactionIsolation"),
                holdability: false,
                clear_warnings: false,
                catalog: capability("catalog"),
                schema: capability("schema"),
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
            closed: false,
            discarded: false,
            savepoint_sequence: 0,
            savepoint_ids: HashMap::new(),
            metadata,
            driver_name,
            standard_executor: None,
            runtime_trace: None,
        })
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    fn client_core(&self) -> Result<&dyn JdbcAgentTransport, RdbcError> {
        self.client.as_deref().ok_or(RdbcError::ConnectionDiscarded)
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
            closed: false,
            discarded: self.discarded,
            savepoint_sequence: self.savepoint_sequence,
            savepoint_ids: HashMap::new(),
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

    async fn execute_update_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<ExecResult, RdbcError> {
        self.ensure_open()?;
        let results = self
            .execute_request(sql, params, StatementGeneratedKeys::AutoGeneratedKeys(1))
            .await?;
        match results.into_iter().next() {
            Some(StatementExecuteResult::Update(result)) => Ok(result),
            Some(StatementExecuteResult::ResultSet(_)) => Err(Self::protocol_error(
                "exec received a result set instead of an update count",
            )),
            None => Err(Self::protocol_error("execute returned no result")),
        }
    }

    async fn execute_query_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, RdbcError> {
        self.ensure_open()?;
        let payload = self
            .client_mut_core()?
            .request(
                "execute_query",
                json!({"sql": sql, "params": Self::parameters(params)?}),
            )
            .await?;
        self.collect_query_payload(payload).await
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
                .client
                .as_deref()
                .is_none_or(JdbcAgentTransport::is_unusable)
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

    fn exec_result(payload: &JsonValue) -> Result<ExecResult, RdbcError> {
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
        Ok(ExecResult {
            rows_affected,
            last_insert_id,
            row_count: None,
        })
    }

    fn generated_keys(generated_keys: StatementGeneratedKeys) -> JsonValue {
        match generated_keys {
            StatementGeneratedKeys::None => json!({"mode": "none"}),
            StatementGeneratedKeys::AutoGeneratedKeys(value) => {
                json!({"mode": "auto", "value": value})
            }
            StatementGeneratedKeys::ColumnIndexes(value) => {
                json!({"mode": "column_indexes", "value": value})
            }
            StatementGeneratedKeys::ColumnNames(value) => {
                json!({"mode": "column_names", "value": value})
            }
        }
    }

    fn statement(
        statement: &dyn PhysicalPreparedStatement,
    ) -> Result<&JdbcAgentPreparedStatement, RdbcError> {
        if statement.is_closed() {
            return Err(RdbcError::ConnectionDiscarded);
        }
        statement
            .as_any()
            .downcast_ref::<JdbcAgentPreparedStatement>()
            .ok_or_else(|| {
                RdbcError::DriverError(
                    "prepared statement was not created by JdbcAgentConnection".to_owned(),
                )
            })
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

    async fn create_savepoint(&mut self, name: Option<&str>) -> Result<Savepoint, RdbcError> {
        self.ensure_open()?;
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        let payload = self
            .client_mut_core()?
            .request("set_savepoint", json!({"name": name}))
            .await?;
        let remote_id = payload
            .get("savepointId")
            .and_then(JsonValue::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Self::protocol_error("set_savepoint response lacks savepointId"))?
            .to_owned();
        let savepoint = Savepoint {
            id: self.savepoint_sequence,
            name: name.map(str::to_owned),
        };
        self.savepoint_ids.insert(savepoint.id, remote_id);
        Ok(savepoint)
    }

    async fn execute_request(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, RdbcError> {
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
            Some("result_set") => Ok(vec![StatementExecuteResult::ResultSet(
                self.collect_query_payload(payload).await?,
            )]),
            Some("update") => Ok(vec![StatementExecuteResult::Update(Self::exec_result(
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
impl PhysicalConnection for JdbcAgentConnection {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        self.execute_update_core(sql, params).await
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, RdbcError> {
        self.ensure_open()?;
        self.execute_request(sql, params, generated_keys).await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        self.execute_query_core(sql, params).await
    }

    async fn fetch_result_set(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, RdbcError> {
        self.ensure_open()?;
        let payload = self
            .client_mut_core()?
            .request(
                "execute_query",
                json!({"sql": sql, "params": Self::parameters(params)?}),
            )
            .await?;
        let labels = Self::column_labels(&payload)?;
        let rows = self.collect_query_payload(payload).await?;
        Ok(Arc::new(RowSetResultSet::with_column_labels(rows, labels)))
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, RdbcError> {
        self.ensure_open()?;
        let result = self
            .client_mut_core()?
            .request(
                "prepare",
                json!({
                    "sql": key.sql(),
                    "generatedKeys": Self::generated_keys(key.statement_generated_keys()),
                }),
            )
            .await?;
        let statement_id = result
            .get("statementId")
            .and_then(JsonValue::as_str)
            .filter(|statement_id| !statement_id.is_empty())
            .ok_or_else(|| Self::protocol_error("prepare response does not contain statementId"))?
            .to_owned();
        let (request_handle, session_id) = self
            .client_core()?
            .statement_context()
            .ok_or_else(|| Self::protocol_error("transport has no physical statement context"))?;
        Ok(Arc::new(JdbcAgentPreparedStatement::new(
            key,
            statement_id,
            session_id,
            request_handle,
        )))
    }

    async fn exec_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, RdbcError> {
        self.ensure_open()?;
        let statement = Self::statement(statement)?;
        let statement_id = statement.statement_id().to_owned();
        let query_timeout_seconds = statement.query_timeout_seconds();
        let generated_keys = statement.generated_keys();
        let payload = self
            .client_mut_core()?
            .request(
                "execute_prepared",
                json!({
                    "statementId": statement_id,
                    "params": Self::parameters(params)?,
                    "mode": "update",
                    "queryTimeoutSeconds": query_timeout_seconds,
                    "generatedKeys": Self::generated_keys(generated_keys),
                }),
            )
            .await?;
        Self::exec_result(&payload)
    }

    async fn exec_prepared_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: Vec<Vec<Value>>,
    ) -> Result<Vec<i32>, RdbcError> {
        self.ensure_open()?;
        let statement = Self::statement(statement)?;
        let parameter_sets = parameter_sets
            .into_iter()
            .map(Self::parameters)
            .collect::<Result<Vec<_>, _>>()?;
        let payload = self
            .client_mut_core()?
            .request(
                "execute_prepared_batch",
                json!({
                    "statementId": statement.statement_id(),
                    "parameterSets": parameter_sets,
                    "queryTimeoutSeconds": statement.query_timeout_seconds(),
                }),
            )
            .await?;
        payload
            .get("updateCounts")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| {
                Self::protocol_error("execute_prepared_batch response lacks updateCounts")
            })?
            .iter()
            .map(|value| {
                value
                    .as_i64()
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or_else(|| {
                        Self::protocol_error(
                            "execute_prepared_batch update count is not a signed 32-bit integer",
                        )
                    })
            })
            .collect()
    }

    async fn execute_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, RdbcError> {
        self.ensure_open()?;
        let statement = Self::statement(statement)?;
        let statement_id = statement.statement_id().to_owned();
        let query_timeout_seconds = statement.query_timeout_seconds();
        let payload = self
            .client_mut_core()?
            .request(
                "execute_prepared",
                json!({
                    "statementId": statement_id,
                    "params": Self::parameters(params)?,
                    "mode": "execute",
                    "queryTimeoutSeconds": query_timeout_seconds,
                    "generatedKeys": Self::generated_keys(generated_keys),
                }),
            )
            .await?;
        match payload.get("kind").and_then(JsonValue::as_str) {
            Some("result_set") => Ok(vec![StatementExecuteResult::ResultSet(
                self.collect_query_payload(payload).await?,
            )]),
            Some("update") => Ok(vec![StatementExecuteResult::Update(Self::exec_result(
                &payload,
            )?)]),
            Some(kind) => Err(Self::protocol_error(format!(
                "unsupported execute_prepared result kind '{kind}'"
            ))),
            None => Err(Self::protocol_error(
                "execute_prepared response does not contain kind",
            )),
        }
    }

    async fn fetch_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, RdbcError> {
        self.ensure_open()?;
        let statement = Self::statement(statement)?;
        let statement_id = statement.statement_id().to_owned();
        let query_timeout_seconds = statement.query_timeout_seconds();
        let payload = self
            .client_mut_core()?
            .request(
                "execute_prepared",
                json!({
                    "statementId": statement_id,
                    "params": Self::parameters(params)?,
                    "mode": "query",
                    "queryTimeoutSeconds": query_timeout_seconds,
                }),
            )
            .await?;
        self.collect_query_payload(payload).await
    }

    async fn fetch_prepared_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, RdbcError> {
        self.ensure_open()?;
        let statement = Self::statement(statement)?;
        let payload = self
            .client_mut_core()?
            .request(
                "execute_prepared",
                json!({
                    "statementId": statement.statement_id(),
                    "params": Self::parameters(params)?,
                    "mode": "query",
                    "queryTimeoutSeconds": statement.query_timeout_seconds(),
                }),
            )
            .await?;
        let labels = Self::column_labels(&payload)?;
        let rows = self.collect_query_payload(payload).await?;
        Ok(Arc::new(RowSetResultSet::with_column_labels(rows, labels)))
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("begin", JsonValue::Null)
            .await?;
        self.auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("commit", JsonValue::Null)
            .await?;
        self.savepoint_ids.clear();
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("rollback", JsonValue::Null)
            .await?;
        self.savepoint_ids.clear();
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let savepoint_id = self
            .savepoint_ids
            .get(&savepoint.id)
            .ok_or_else(|| {
                RdbcError::InvalidArgument(format!("unknown JDBC Agent savepoint {}", savepoint.id))
            })?
            .clone();
        self.client_mut_core()?
            .request(
                "rollback_to_savepoint",
                json!({"savepointId": savepoint_id}),
            )
            .await?;
        Ok(())
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        self.create_savepoint(None).await
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, RdbcError> {
        if name.is_empty() {
            return Err(RdbcError::InvalidArgument(
                "JDBC savepoint name must not be empty".to_owned(),
            ));
        }
        self.create_savepoint(Some(name)).await
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.ensure_open()?;
        let savepoint_id = self.savepoint_ids.remove(&savepoint.id).ok_or_else(|| {
            RdbcError::InvalidArgument(format!("unknown JDBC Agent savepoint {}", savepoint.id))
        })?;
        self.client_mut_core()?
            .request("release_savepoint", json!({"savepointId": savepoint_id}))
            .await?;
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("validate_connection", JsonValue::Null)
            .await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.close_connection_core().await
    }

    fn is_closed(&self) -> bool {
        self.closed
            || self
                .client
                .as_deref()
                .is_none_or(JdbcAgentTransport::is_unusable)
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        self.capabilities
    }

    fn database_meta_data(&mut self) -> Result<Box<dyn PhysicalDatabaseMetaData + '_>, RdbcError> {
        self.ensure_open()?;
        Ok(Box::new(self.metadata.clone()))
    }

    fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("set_auto_commit", json!({"value": auto_commit}))
            .await?;
        self.auto_commit = auto_commit;
        Ok(())
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("set_read_only", json!({"value": read_only}))
            .await?;
        self.read_only = read_only;
        Ok(())
    }

    fn transaction_isolation(&self) -> u8 {
        self.transaction_isolation
    }

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("set_transaction_isolation", json!({"value": level}))
            .await?;
        self.transaction_isolation = level;
        Ok(())
    }

    fn catalog(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    async fn set_catalog(&mut self, catalog: &str) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("set_catalog", json!({"value": catalog}))
            .await?;
        self.catalog = Some(catalog.to_owned());
        Ok(())
    }

    fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.client_mut_core()?
            .request("set_schema", json!({"value": schema}))
            .await?;
        self.schema = Some(schema.to_owned());
        Ok(())
    }

    fn mark_discarded(&mut self) {
        self.discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.discarded
            || self
                .client
                .as_deref()
                .is_none_or(JdbcAgentTransport::is_unusable)
    }

    fn driver_name(&self) -> &str {
        &self.driver_name
    }
}

#[async_trait::async_trait]
impl AdapterExecutionCore for JdbcAgentConnection {
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
        let mut snapshot = self.metadata.clone();
        let product_name =
            druid_rdbc::metadata::DatabaseMetaData::database_product_name(&mut snapshot)
                .await
                .ok();
        Ok(Box::new(StandardAdapterDatabaseMetaData::new(
            self.standard_executor_core()?,
            product_name,
            "jdbc_agent_database_metadata_product_name_absent",
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
}
