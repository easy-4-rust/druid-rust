//! JDBC Agent 会话内的标准 CallableStatement 实体。

use super::jdbc_agent_cursor_guard::JdbcAgentCursorGuard;
use super::AgentValue;
use crate::standard_adapter_executor::StandardExecutor;
use crate::standard_adapter_result_set::StandardAdapterResultSet;
use crate::standard_result_rows::StandardResultRows;
use crate::standard_statement_state::{
    delegate_standard_statement_state_methods, StandardStatementState,
};
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ExecutionResult, ParameterIndex, PreparedInputParameter, PreparedParameter, PreparedStatement,
    Statement, StatementOptions,
};
use druid_rdbc::types::{RdbcObject, Value};
use serde_json::{json, Value as JsonValue};
use std::cell::Cell;
use std::sync::Arc;

/// 持有单个 Agent statementId，所有请求经创建连接的共享 executor 串行进入同一 session。
/// 对应 Java: java.sql.CallableStatement。
pub(crate) struct JdbcAgentCallableStatement {
    sql: String,
    statement_id: String,
    executor: Option<Arc<dyn StandardExecutor>>,
    state: StandardStatementState,
    inputs: Vec<(CallableParameter, Value)>,
    outputs: Vec<(CallableParameter, CallableOutParameter)>,
    out_values: Vec<(CallableParameter, Value)>,
    was_null: Cell<bool>,
}

impl JdbcAgentCallableStatement {
    /// 建立与已成功创建的远程句柄一一对应的逻辑所有者。
    pub(crate) fn new(
        sql: String,
        statement_id: String,
        options: StatementOptions,
        executor: Arc<dyn StandardExecutor>,
    ) -> Self {
        Self {
            sql,
            statement_id,
            executor: Some(executor),
            state: StandardStatementState::new(options),
            inputs: Vec::new(),
            outputs: Vec::new(),
            out_values: Vec::new(),
            was_null: Cell::new(false),
        }
    }

    fn executor_core(&self) -> Result<&Arc<dyn StandardExecutor>, RdbcError> {
        let executor = self
            .executor
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?;
        executor.ensure_open()?;
        Ok(executor)
    }

    fn replace_input(&mut self, selector: CallableParameter, value: Value) {
        if let Some((_, prior)) = self.inputs.iter_mut().find(|(key, _)| *key == selector) {
            *prior = value;
        } else {
            self.inputs.push((selector, value));
        }
    }

    fn selector_json(selector: &CallableParameter) -> JsonValue {
        match selector {
            CallableParameter::Index(index) => json!({"index": index.get()}),
            CallableParameter::Name(name) => json!({"name": name}),
        }
    }

    fn protocol_error(message: impl Into<String>) -> RdbcError {
        super::JdbcAgentConnection::protocol_error(message)
    }

    async fn collect_rows(
        executor: &Arc<dyn StandardExecutor>,
        payload: JsonValue,
        remaining_budget: &mut u64,
    ) -> Result<(Vec<String>, StandardResultRows, JdbcAgentCursorGuard), RdbcError> {
        let cursor_id = payload
            .get("cursorId")
            .and_then(JsonValue::as_str)
            .map(str::to_owned);
        let mut guard = JdbcAgentCursorGuard::new(Arc::clone(executor), cursor_id);
        let result = Self::collect_rows_inner(executor, payload, remaining_budget).await;
        if result.is_err() {
            // 保留原始错误；清理失败不可掩盖协议或驱动错误。
            let _ = guard.close_now().await;
        }
        result.map(|(labels, rows)| (labels, rows, guard))
    }

    async fn collect_rows_inner(
        executor: &Arc<dyn StandardExecutor>,
        mut payload: JsonValue,
        remaining_budget: &mut u64,
    ) -> Result<(Vec<String>, StandardResultRows), RdbcError> {
        let labels = payload
            .get("columns")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| Self::protocol_error("callable result has no columns"))?
            .iter()
            .map(|column| {
                column
                    .get("label")
                    .and_then(JsonValue::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| Self::protocol_error("callable column has no label"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut rows = StandardResultRows::new_spool()?;
        loop {
            let page: Vec<Vec<AgentValue>> = serde_json::from_value(
                payload
                    .get("rows")
                    .cloned()
                    .ok_or_else(|| Self::protocol_error("callable page has no rows"))?,
            )
            .map_err(|error| Self::protocol_error(error.to_string()))?;
            for values in page {
                // 保持执行期解码错误的优先级，避免返回的游标在读取时才暴露损坏值。
                for value in &values {
                    value.clone().into_druid()?;
                }
                rows.push_agent_row(values, remaining_budget)?;
            }
            if !payload
                .get("hasMore")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false)
            {
                break;
            }
            let cursor_id = payload
                .get("cursorId")
                .and_then(JsonValue::as_str)
                .filter(|id| !id.is_empty())
                .ok_or_else(|| Self::protocol_error("callable page hasMore without cursorId"))?;
            payload = executor
                .connection_control(
                    "fetch_page",
                    json!({
                        "cursorId": cursor_id, "pageSize": 500,
                    }),
                )
                .await?;
        }
        Ok((labels, rows))
    }
}

druid_rdbc::impl_standard_wrapper!(JdbcAgentCallableStatement => CallableStatement);

#[async_trait::async_trait]
impl Statement for JdbcAgentCallableStatement {
    delegate_standard_statement_state_methods!();

    fn get_update_count(&self) -> Result<Option<i64>, RdbcError> {
        self.executor_core()?;
        Ok(self.state.last_update_count)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.executor.is_none() {
            return Ok(());
        }
        let executor = Arc::clone(self.executor_core()?);
        executor
            .connection_control(
                "close_statement",
                json!({
                    "statementId": self.statement_id,
                }),
            )
            .await?;
        self.out_values.clear();
        self.inputs.clear();
        self.outputs.clear();
        self.executor.take();
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.executor.is_none()
            || self
                .executor
                .as_ref()
                .is_some_and(|executor| executor.is_closed()))
    }
}

#[async_trait::async_trait]
impl PreparedStatement for JdbcAgentCallableStatement {
    fn sql(&self) -> Result<&str, RdbcError> {
        self.executor_core()?;
        Ok(&self.sql)
    }

    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.replace_input(CallableParameter::Index(parameter.index), parameter.value);
        Ok(())
    }

    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.executor_core()?;
        let value = match parameter {
            PreparedInputParameter::RustValue(value) => value,
            PreparedInputParameter::Boolean(value) => Value::Bool(value),
            PreparedInputParameter::Byte(value) => Value::Int(i64::from(value)),
            PreparedInputParameter::Short(value) => Value::Int(i64::from(value)),
            PreparedInputParameter::Int(value) => Value::Int(i64::from(value)),
            PreparedInputParameter::Long(value) => Value::Int(value),
            PreparedInputParameter::String(value) => value.map_or(Value::Null, Value::String),
            _ => {
                return Err(RdbcError::feature_not_supported(
                    "jdbc_agent_callable_rich_indexed_input",
                ))
            }
        };
        self.replace_input(CallableParameter::Index(index), value);
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.executor_core()?;
        self.inputs.clear();
        Ok(())
    }

    async fn execute_prepared(&mut self) -> Result<Vec<ExecutionResult>, RdbcError> {
        let executor = Arc::clone(self.executor_core()?);
        self.out_values.clear();
        self.was_null.set(false);
        self.state.last_update_count = None;
        let inputs = self
            .inputs
            .iter()
            .map(|(selector, value)| {
                let mut item = Self::selector_json(selector);
                item["value"] = serde_json::to_value(AgentValue::from_druid(value.clone())?)
                    .map_err(|error| Self::protocol_error(error.to_string()))?;
                Ok(item)
            })
            .collect::<Result<Vec<_>, RdbcError>>()?;
        let outputs = self
            .outputs
            .iter()
            .map(|(selector, descriptor)| {
                let mut item = Self::selector_json(selector);
                item["sqlType"] = json!(descriptor.sql_type());
                if let Some(scale) = descriptor.scale() {
                    item["scale"] = json!(scale);
                }
                if let Some(type_name) = descriptor.type_name() {
                    item["typeName"] = json!(type_name);
                }
                item
            })
            .collect::<Vec<_>>();
        let payload = executor
            .connection_control(
                "execute_callable",
                json!({
                    "statementId": self.statement_id,
                    "inParameters": inputs,
                    "outParameters": outputs,
                    "queryTimeoutSeconds": self.state.query_timeout,
                    "maxRows": self.state.max_rows,
                }),
            )
            .await?;
        let mut initial_cursor_guard = JdbcAgentCursorGuard::new(
            Arc::clone(&executor),
            payload
                .get("cursorId")
                .and_then(JsonValue::as_str)
                .map(str::to_owned),
        );
        let response_values = payload
            .get("outValues")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| Self::protocol_error("callable response has no outValues"))?;
        if response_values.len() != self.outputs.len() {
            return Err(Self::protocol_error("callable OUT count mismatch"));
        }
        let mut values = Vec::with_capacity(response_values.len());
        for (descriptor, (selector, _)) in response_values.iter().zip(&self.outputs) {
            let observed = match selector {
                CallableParameter::Index(index) => {
                    descriptor.get("index") == Some(&json!(index.get()))
                }
                CallableParameter::Name(name) => descriptor.get("name") == Some(&json!(name)),
            };
            if !observed {
                return Err(Self::protocol_error("callable OUT selector mismatch"));
            }
            let encoded: AgentValue = serde_json::from_value(
                descriptor
                    .get("value")
                    .cloned()
                    .ok_or_else(|| Self::protocol_error("callable OUT has no value"))?,
            )
            .map_err(|error| Self::protocol_error(error.to_string()))?;
            let value = encoded.into_druid()?;
            let was_null = descriptor
                .get("wasNull")
                .and_then(JsonValue::as_bool)
                .ok_or_else(|| Self::protocol_error("callable OUT has no wasNull"))?;
            if was_null != matches!(value, Value::Null) {
                return Err(Self::protocol_error("callable OUT null status mismatch"));
            }
            values.push((selector.clone(), value));
        }
        let mut results = Vec::new();
        let mut first_update_count = None;
        // 一次执行的所有结果集共享磁盘预算，避免多个小结果绕过单结果限制。
        let mut remaining_spool_budget = 1024_u64 * 1024 * 1024;
        let mut current_payload = payload;
        loop {
            let mut cursor_guard = None;
            match current_payload.get("kind").and_then(JsonValue::as_str) {
                Some("done") => break,
                Some("none") => {}
                Some("update") => {
                    let count = current_payload
                        .get("rowsAffected")
                        .and_then(JsonValue::as_i64)
                        .ok_or_else(|| {
                            Self::protocol_error("callable update has no rowsAffected")
                        })?;
                    if results.is_empty() {
                        first_update_count = Some(count);
                    }
                    results.push(ExecutionResult::UpdateCount(count));
                }
                Some("result_set") => {
                    // 首次响应在 OUT 校验阶段也受守卫保护；分页接手前没有异步悬空窗口。
                    initial_cursor_guard.disarm();
                    let (labels, rows, guard) =
                        Self::collect_rows(&executor, current_payload, &mut remaining_spool_budget)
                            .await?;
                    cursor_guard = Some(guard);
                    results.push(ExecutionResult::ResultSet(Box::new(
                        StandardAdapterResultSet::new_spooled(labels, rows, Arc::clone(&executor)),
                    )));
                }
                _ => {
                    return Err(Self::protocol_error(
                        "callable response has no supported kind",
                    ))
                }
            }
            current_payload = executor
                .connection_control(
                    "next_callable_result",
                    json!({"statementId": self.statement_id}),
                )
                .await?;
            // Java getMoreResults 已关闭上一游标；清理守卫只覆盖推进前取消窗口。
            if let Some(mut guard) = cursor_guard {
                guard.disarm();
            }
        }
        self.state.last_update_count = first_update_count;
        self.out_values = values;
        Ok(results)
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        let results = self.execute_prepared().await?;
        match results.as_slice() {
            [ExecutionResult::UpdateCount(count)] => Ok(*count),
            _ => Err(Self::protocol_error(
                "callable update returned a mixed or non-update result sequence",
            )),
        }
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        let mut results = self.execute_prepared().await?;
        if results.len() != 1 {
            return Err(Self::protocol_error(
                "callable query returned a mixed or multiple result sequence",
            ));
        }
        match results.pop() {
            Some(ExecutionResult::ResultSet(result)) => Ok(result),
            _ => Err(Self::protocol_error("callable query returned update count")),
        }
    }
}

#[async_trait::async_trait]
impl CallableStatement for JdbcAgentCallableStatement {
    fn register_out_parameter(
        &mut self,
        selector: CallableParameter,
        descriptor: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.executor_core()?;
        if let Some((_, old)) = self.outputs.iter_mut().find(|(key, _)| *key == selector) {
            *old = descriptor;
        } else {
            self.outputs.push((selector, descriptor));
        }
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        self.executor_core()?;
        let selector = CallableParameter::by_name(name)?;
        let value = match parameter {
            CallableInputParameter::Boolean(value) => Value::Bool(value),
            CallableInputParameter::Byte(value) => Value::Int(i64::from(value)),
            CallableInputParameter::Short(value) => Value::Int(i64::from(value)),
            CallableInputParameter::Int(value) => Value::Int(i64::from(value)),
            CallableInputParameter::Long(value) => Value::Int(value),
            CallableInputParameter::String(value) => value.map_or(Value::Null, Value::String),
            CallableInputParameter::Object {
                value,
                target_sql_type: None,
                scale: None,
            } => value,
            _ => {
                return Err(RdbcError::feature_not_supported(
                    "jdbc_agent_callable_rich_named_input",
                ))
            }
        };
        self.replace_input(selector, value);
        Ok(())
    }

    fn out_parameter(&self, selector: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        self.executor_core()?;
        let value = self
            .out_values
            .iter()
            .find(|(key, _)| key == selector)
            .map(|(_, value)| value)
            .ok_or_else(|| {
                RdbcError::InvalidArgument("OUT parameter has no current value".to_owned())
            })?;
        self.was_null.set(matches!(value, Value::Null));
        Ok(RdbcObject::Scalar(value.clone()))
    }

    fn out_value(&self, index: ParameterIndex) -> Result<Value, RdbcError> {
        let object = self.out_parameter(&CallableParameter::Index(index))?;
        match object {
            RdbcObject::Scalar(value) => Ok(value),
            _ => Err(Self::protocol_error("callable OUT was not scalar")),
        }
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.executor_core()?;
        Ok(self.was_null.get())
    }
}
