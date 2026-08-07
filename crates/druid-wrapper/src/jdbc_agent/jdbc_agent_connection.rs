use super::{AgentValue, JdbcAgentClient, JdbcAgentOptions, JdbcAgentPreparedStatement};
use druid::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalPreparedStatement, PreparedStatementKey, Row, StatementExecuteResult,
    StatementGeneratedKeys, Value,
};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::sync::Arc;

/// 由 Druid 独占并通过 JDBC Agent 操作的未池化物理连接。
#[allow(clippy::struct_excessive_bools)]
pub struct JdbcAgentConnection {
    client: JdbcAgentClient,
    capabilities: PhysicalConnectionCapabilities,
    auto_commit: bool,
    read_only: bool,
    transaction_isolation: u8,
    catalog: Option<String>,
    schema: Option<String>,
    closed: bool,
    discarded: bool,
}

impl JdbcAgentConnection {
    /// 启动 Agent，并用显式 JDBC URL、属性和验证 SQL 建立一个物理连接。
    pub async fn connect(
        url: &str,
        validation_query: Option<&str>,
        properties: HashMap<String, String>,
        options: JdbcAgentOptions,
    ) -> Result<Self, DruidError> {
        let (client, session) = JdbcAgentClient::connect(
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
        Ok(Self {
            client,
            capabilities: PhysicalConnectionCapabilities {
                transactions: capability("transactions"),
                savepoints: false,
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
        })
    }

    fn ensure_open(&self) -> Result<(), DruidError> {
        if self.closed || self.discarded || self.client.is_unusable() {
            Err(DruidError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn parameters(params: Vec<Value>) -> Result<Vec<AgentValue>, DruidError> {
        params.into_iter().map(AgentValue::from_druid).collect()
    }

    fn rows(payload: &JsonValue) -> Result<Vec<Row>, DruidError> {
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

    async fn collect_query_payload(
        &mut self,
        mut payload: JsonValue,
    ) -> Result<Vec<Row>, DruidError> {
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
                            .client
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
                .client
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
                        .client
                        .request("close_cursor", json!({"cursorId": cursor_id}))
                        .await;
                    return Err(error);
                }
            };
        }
    }

    fn exec_result(payload: &JsonValue) -> Result<ExecResult, DruidError> {
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
    ) -> Result<&JdbcAgentPreparedStatement, DruidError> {
        if statement.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }
        statement
            .as_any()
            .downcast_ref::<JdbcAgentPreparedStatement>()
            .ok_or_else(|| {
                DruidError::DriverError(
                    "prepared statement was not created by JdbcAgentConnection".to_owned(),
                )
            })
    }

    fn protocol_error(message: impl Into<String>) -> DruidError {
        DruidError::DriverError(format!("invalid JDBC Agent response: {}", message.into()))
    }

    async fn execute_request(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let payload = self
            .client
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
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
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

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        self.ensure_open()?;
        self.execute_request(sql, params, generated_keys).await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.ensure_open()?;
        let payload = self
            .client
            .request(
                "execute_query",
                json!({"sql": sql, "params": Self::parameters(params)?}),
            )
            .await?;
        self.collect_query_payload(payload).await
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.ensure_open()?;
        let result = self
            .client
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
        let (request_handle, session_id) = self.client.statement_context();
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
    ) -> Result<ExecResult, DruidError> {
        self.ensure_open()?;
        let statement_id = Self::statement(statement)?.statement_id().to_owned();
        let payload = self
            .client
            .request(
                "execute_prepared",
                json!({
                    "statementId": statement_id,
                    "params": Self::parameters(params)?,
                    "mode": "update",
                }),
            )
            .await?;
        Self::exec_result(&payload)
    }

    async fn execute_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        self.ensure_open()?;
        let statement_id = Self::statement(statement)?.statement_id().to_owned();
        let payload = self
            .client
            .request(
                "execute_prepared",
                json!({
                    "statementId": statement_id,
                    "params": Self::parameters(params)?,
                    "mode": "execute",
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
    ) -> Result<Vec<Row>, DruidError> {
        self.ensure_open()?;
        let statement_id = Self::statement(statement)?.statement_id().to_owned();
        let payload = self
            .client
            .request(
                "execute_prepared",
                json!({
                    "statementId": statement_id,
                    "params": Self::parameters(params)?,
                    "mode": "query",
                }),
            )
            .await?;
        self.collect_query_payload(payload).await
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client.request("begin", JsonValue::Null).await?;
        self.auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client.request("commit", JsonValue::Null).await?;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client.request("rollback", JsonValue::Null).await?;
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client
            .request("validate_connection", JsonValue::Null)
            .await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        if self.closed {
            return Ok(());
        }
        let result = self.client.close().await;
        self.closed = true;
        result
    }

    fn is_closed(&self) -> bool {
        self.closed || self.client.is_unusable()
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        self.capabilities
    }

    fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client
            .request("set_auto_commit", json!({"value": auto_commit}))
            .await?;
        self.auto_commit = auto_commit;
        Ok(())
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client
            .request("set_read_only", json!({"value": read_only}))
            .await?;
        self.read_only = read_only;
        Ok(())
    }

    fn transaction_isolation(&self) -> u8 {
        self.transaction_isolation
    }

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client
            .request("set_transaction_isolation", json!({"value": level}))
            .await?;
        self.transaction_isolation = level;
        Ok(())
    }

    fn catalog(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    async fn set_catalog(&mut self, catalog: &str) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client
            .request("set_catalog", json!({"value": catalog}))
            .await?;
        self.catalog = Some(catalog.to_owned());
        Ok(())
    }

    fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        self.ensure_open()?;
        self.client
            .request("set_schema", json!({"value": schema}))
            .await?;
        self.schema = Some(schema.to_owned());
        Ok(())
    }

    fn mark_discarded(&mut self) {
        self.discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.discarded || self.client.is_unusable()
    }
}
