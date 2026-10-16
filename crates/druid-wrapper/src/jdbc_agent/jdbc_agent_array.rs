use super::agent_value::AgentValue;
use super::jdbc_agent_connection::JdbcAgentConnection;
use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{Array, ResourceOperations};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::types::{RdbcObject, RdbcTypeMap};
use serde_json::{json, Value as JsonValue};
use std::sync::Arc;

/// 由单个 JDBC Agent session 拥有的远程 SQL Array。对应 Java: java.sql.Array。
pub(super) struct JdbcAgentArray {
    executor: Arc<dyn StandardExecutor>,
    array_id: String,
    freed: bool,
}

impl JdbcAgentArray {
    /// 保存已由 Agent 注册的 Array 句柄；参数为串行执行器和不透明标识。
    pub(super) fn new(executor: Arc<dyn StandardExecutor>, array_id: String) -> Self {
        Self {
            executor,
            array_id,
            freed: false,
        }
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.freed || self.executor.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    async fn metadata(&self) -> Result<JsonValue, RdbcError> {
        self.ensure_open()?;
        self.executor
            .connection_control("array_metadata", json!({"arrayId": self.array_id}))
            .await
    }

    async fn read_values(&self, range: Option<(i64, i32)>) -> Result<Vec<RdbcObject>, RdbcError> {
        self.ensure_open()?;
        let payload = match range {
            Some((index, count)) => {
                json!({"arrayId": self.array_id, "index": index, "count": count})
            }
            None => json!({"arrayId": self.array_id}),
        };
        let response = self
            .executor
            .connection_control("array_values", payload)
            .await?;
        let values = response.get("values").cloned().ok_or_else(|| {
            JdbcAgentConnection::protocol_error("array_values returned no values")
        })?;
        let values: Vec<AgentValue> = serde_json::from_value(values)
            .map_err(|error| JdbcAgentConnection::protocol_error(error.to_string()))?;
        values.into_iter().map(AgentValue::into_object).collect()
    }
}

impl std::fmt::Debug for JdbcAgentArray {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentArray")
            .field("array_id", &self.array_id)
            .field("freed", &self.freed)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl Array for JdbcAgentArray {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::RANGE | ResourceOperations::FREE
    }

    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        self.metadata()
            .await?
            .get("baseTypeName")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
            .ok_or_else(|| {
                JdbcAgentConnection::protocol_error("array_metadata returned no baseTypeName")
            })
    }

    async fn base_type(&mut self) -> Result<i32, RdbcError> {
        self.metadata()
            .await?
            .get("baseType")
            .and_then(JsonValue::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| {
                JdbcAgentConnection::protocol_error("array_metadata returned no baseType")
            })
    }

    async fn values(&mut self) -> Result<Vec<RdbcObject>, RdbcError> {
        self.read_values(None).await
    }

    async fn values_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.ensure_open()?;
        Err(RdbcError::feature_not_supported("Array#getArray(Map)"))
    }

    async fn values_range(&mut self, index: i64, count: i32) -> Result<Vec<RdbcObject>, RdbcError> {
        self.read_values(Some((index, count))).await
    }

    async fn values_range_with_type_map(
        &mut self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.ensure_open()?;
        Err(RdbcError::feature_not_supported(
            "Array#getArray(long,int,Map)",
        ))
    }

    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        Err(RdbcError::feature_not_supported("Array#getResultSet"))
    }

    async fn result_set_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        Err(RdbcError::feature_not_supported("Array#getResultSet(Map)"))
    }

    async fn result_set_range(
        &mut self,
        _index: i64,
        _count: i32,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        Err(RdbcError::feature_not_supported(
            "Array#getResultSet(long,int)",
        ))
    }

    async fn result_set_range_with_type_map(
        &mut self,
        _index: i64,
        _count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        Err(RdbcError::feature_not_supported(
            "Array#getResultSet(long,int,Map)",
        ))
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        if self.freed {
            return Ok(());
        }
        self.ensure_open()?;
        self.executor
            .connection_control("free_array", json!({"arrayId": self.array_id}))
            .await?;
        self.freed = true;
        Ok(())
    }
}
