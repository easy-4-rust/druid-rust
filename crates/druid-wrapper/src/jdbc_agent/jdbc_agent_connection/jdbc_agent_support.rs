use super::*;

pub(crate) enum JdbcAgentExecutionResult {
    Query,
    Update(AdapterUpdateResult),
}

#[async_trait::async_trait]
pub(crate) trait JdbcAgentTransport: Send {
    async fn request(
        &mut self,
        operation: &str,
        payload: JsonValue,
    ) -> Result<JsonValue, RdbcError>;
    async fn close(&mut self) -> Result<(), RdbcError>;
    fn is_unusable(&self) -> bool;
    fn cancellation_context(&self) -> Option<Arc<JdbcAgentCancellationContext>> {
        None
    }
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

    fn cancellation_context(&self) -> Option<Arc<JdbcAgentCancellationContext>> {
        Some(JdbcAgentClient::cancellation_context(self))
    }
}

#[async_trait::async_trait]
pub(crate) trait JdbcAgentConnector: Send + Sync {
    async fn connect(
        &self,
        options: JdbcAgentOptions,
        payload: JsonValue,
    ) -> Result<(Box<dyn JdbcAgentTransport>, JsonValue), RdbcError>;
}

pub(crate) struct NativeJdbcAgentConnector;

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
    pub(crate) client: Option<Box<dyn JdbcAgentTransport>>,
    pub(crate) cancellation: Option<Arc<JdbcAgentCancellationContext>>,
    pub(crate) capabilities: druid_rdbc::extensions::ConnectionCapabilities,
    pub(crate) auto_commit: bool,
    pub(crate) read_only: bool,
    pub(crate) transaction_isolation: u8,
    pub(crate) catalog: Option<String>,
    pub(crate) schema: Option<String>,
    pub(crate) savepoints: HashMap<u64, String>,
    pub(crate) next_savepoint_id: u64,
    pub(crate) closed: bool,
    pub(crate) discarded: bool,
    pub(crate) metadata: JdbcAgentDatabaseMetaData,
    pub(crate) driver_name: String,
    pub(crate) standard_executor: Option<Arc<dyn StandardExecutor>>,
    pub(crate) runtime_trace: Option<RuntimeTraceEmitter>,
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

    pub(crate) async fn connect_with_connector(
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
        Ok(Self::from_session(client, session))
    }

    pub(crate) fn from_session(client: Box<dyn JdbcAgentTransport>, session: JsonValue) -> Self {
        let cancellation = client.cancellation_context();
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
        Self {
            client: Some(client),
            cancellation,
            capabilities: druid_rdbc::extensions::ConnectionCapabilities {
                transactions: supports_transactions,
                // 保存点只能存在于事务中；协议返回矛盾能力时按可执行的交集声明。
                savepoints: supports_transactions && capability("savepoints"),
                catalogs: capability("catalog"),
                schemas: capability("schema"),
                callable_statements: capability("callableStatements"),
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
        }
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    pub(crate) fn client_mut_core(
        &mut self,
    ) -> Result<&mut (dyn JdbcAgentTransport + '_), RdbcError> {
        match self.client.as_deref_mut() {
            Some(client) => Ok(client),
            None => Err(RdbcError::ConnectionDiscarded),
        }
    }

    pub(crate) fn standard_executor_core(
        &mut self,
    ) -> Result<Arc<dyn StandardExecutor>, RdbcError> {
        if let Some(executor) = &self.standard_executor {
            return Ok(Arc::clone(executor));
        }
        self.ensure_open()?;
        let runtime_trace = self.runtime_trace.take();
        let shell = Self {
            client: None,
            cancellation: self.cancellation.clone(),
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

    pub(crate) async fn create_savepoint(
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

    pub(crate) async fn execute_update_core(
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

    pub(crate) async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
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

    pub(crate) fn ensure_open(&self) -> Result<(), RdbcError> {
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

    pub(crate) fn parameters(params: Vec<Value>) -> Result<Vec<AgentValue>, RdbcError> {
        params.into_iter().map(AgentValue::from_druid).collect()
    }

    pub(crate) fn rows(payload: &JsonValue) -> Result<Vec<Row>, RdbcError> {
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

    pub(crate) fn column_labels(payload: &JsonValue) -> Result<Vec<String>, RdbcError> {
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

    pub(crate) async fn collect_query_payload(
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

    pub(crate) fn exec_result(payload: &JsonValue) -> Result<AdapterUpdateResult, RdbcError> {
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

    pub(crate) fn generated_keys(
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> JsonValue {
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

    pub(crate) fn protocol_error(message: impl Into<String>) -> RdbcError {
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

    pub(crate) async fn execute_request(
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
    fn is_unusable_core(&self) -> bool {
        self.closed
            || self.discarded
            || self
                .client
                .as_ref()
                .is_none_or(|client| client.is_unusable())
    }

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
