use super::agent_value::AgentValue;
use super::jdbc_agent_connection::JdbcAgentConnection;
use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, ResultSetColumnType, ResultSetMetaData,
    ResultSetMetaDataSnapshot,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{ResultSetConcurrency, ResultSetType};
use druid_rdbc::types::{RdbcObject, Value};
use serde_json::{json, Value as JsonValue};
use std::cell::Cell;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

type PendingPage = Pin<Box<dyn Future<Output = Result<JsonValue, RdbcError>> + Send>>;

/// 按需读取 JDBC Agent Array 子游标的前向结果集。对应 Java: java.sql.ResultSet。
pub(super) struct JdbcAgentArrayResultSet {
    executor: Arc<dyn StandardExecutor>,
    parent_active: Arc<AtomicBool>,
    cursor_id: Option<String>,
    rows: VecDeque<Vec<AgentValue>>,
    pending: Option<PendingPage>,
    metadata: Arc<ResultSetMetaDataSnapshot>,
    current: Option<Vec<AgentValue>>,
    row_number: i32,
    was_null: Cell<bool>,
    has_more: bool,
    closed: bool,
}

impl JdbcAgentArrayResultSet {
    /// 以原驱动 Array 游标首个有界页构造标准结果集；后续页在 next 时读取。
    pub(super) async fn open(
        executor: Arc<dyn StandardExecutor>,
        parent_active: Arc<AtomicBool>,
        array_id: &str,
        range: Option<(i64, i32)>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let payload = match range {
            Some((index, count)) => {
                json!({"arrayId": array_id, "index": index, "count": count, "pageSize": 8})
            }
            None => json!({"arrayId": array_id, "pageSize": 8}),
        };
        let response = executor
            .connection_control("array_result_set", payload)
            .await?;
        let cursor_id = Self::cursor_id(&response)?;
        let metadata = match Self::metadata(&response) {
            Ok(metadata) => metadata,
            Err(error) => {
                Self::close_remote(&executor, cursor_id.as_deref()).await;
                return Err(error);
            }
        };
        let rows = match Self::rows(&response) {
            Ok(rows) => rows,
            Err(error) => {
                Self::close_remote(&executor, cursor_id.as_deref()).await;
                return Err(error);
            }
        };
        Ok(Box::new(Self {
            executor,
            parent_active,
            cursor_id,
            rows,
            pending: None,
            metadata: Arc::new(metadata),
            current: None,
            row_number: 0,
            was_null: Cell::new(false),
            has_more: response
                .get("hasMore")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false),
            closed: false,
        }))
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed || !self.parent_active.load(Ordering::Acquire) || self.executor.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    fn cursor_id(response: &JsonValue) -> Result<Option<String>, RdbcError> {
        let has_more = response
            .get("hasMore")
            .and_then(JsonValue::as_bool)
            .ok_or_else(|| JdbcAgentConnection::protocol_error("Array cursor has no hasMore"))?;
        let cursor_id = response
            .get("cursorId")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .map(str::to_owned);
        if has_more && cursor_id.is_none() {
            return Err(JdbcAgentConnection::protocol_error(
                "Array cursor hasMore without cursorId",
            ));
        }
        Ok(cursor_id)
    }

    fn rows(response: &JsonValue) -> Result<VecDeque<Vec<AgentValue>>, RdbcError> {
        let encoded = response
            .get("rows")
            .cloned()
            .ok_or_else(|| JdbcAgentConnection::protocol_error("Array cursor has no rows"))?;
        serde_json::from_value::<Vec<Vec<AgentValue>>>(encoded)
            .map(VecDeque::from)
            .map_err(|error| JdbcAgentConnection::protocol_error(error.to_string()))
    }

    fn metadata(response: &JsonValue) -> Result<ResultSetMetaDataSnapshot, RdbcError> {
        let columns = response
            .get("columns")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| JdbcAgentConnection::protocol_error("Array cursor has no columns"))?;
        let descriptors = columns
            .iter()
            .map(|column| {
                let label = column
                    .get("label")
                    .and_then(JsonValue::as_str)
                    .ok_or_else(|| {
                        JdbcAgentConnection::protocol_error("Array cursor column has no label")
                    })?;
                let code = column
                    .get("jdbcType")
                    .and_then(JsonValue::as_i64)
                    .and_then(|value| i32::try_from(value).ok())
                    .ok_or_else(|| {
                        JdbcAgentConnection::protocol_error("Array cursor column has no jdbcType")
                    })?;
                Ok(
                    ColumnDescriptor::new(label, "OTHER", ColumnNullability::Unknown)
                        .with_driver_type(label, ResultSetColumnType::Jdbc(code)),
                )
            })
            .collect::<Result<Vec<_>, RdbcError>>()?;
        Ok(ResultSetMetaDataSnapshot::new(descriptors))
    }

    async fn close_remote(executor: &Arc<dyn StandardExecutor>, cursor_id: Option<&str>) {
        if let Some(cursor_id) = cursor_id {
            let _ = executor
                .connection_control("close_cursor", json!({"cursorId": cursor_id}))
                .await;
        }
    }

    fn current_value(&self, column: ColumnIndex) -> Result<AgentValue, RdbcError> {
        self.ensure_open()?;
        self.current
            .as_ref()
            .ok_or_else(|| {
                RdbcError::InvalidArgument("Array ResultSet is not on a row".to_owned())
            })?
            .get(column.get() - 1)
            .cloned()
            .ok_or_else(|| {
                RdbcError::InvalidArgument(format!(
                    "Array ResultSet column {} is out of range",
                    column.get()
                ))
            })
    }
}

impl Drop for JdbcAgentArrayResultSet {
    fn drop(&mut self) {
        if let Some(cursor_id) = self.cursor_id.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                let executor = Arc::clone(&self.executor);
                runtime.spawn(async move {
                    Self::close_remote(&executor, Some(&cursor_id)).await;
                });
            }
        }
    }
}

druid_rdbc::impl_standard_wrapper!(JdbcAgentArrayResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for JdbcAgentArrayResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        loop {
            if let Some(row) = self.rows.pop_front() {
                self.row_number = self.row_number.checked_add(1).ok_or_else(|| {
                    RdbcError::InvalidArgument("Array cursor row exceeds i32".to_owned())
                })?;
                self.current = Some(row);
                self.was_null.set(false);
                return Ok(true);
            }
            self.current = None;
            if !self.has_more {
                return Ok(false);
            }
            if self.pending.is_none() {
                let executor = Arc::clone(&self.executor);
                let cursor_id = self.cursor_id.clone().ok_or_else(|| {
                    JdbcAgentConnection::protocol_error("Array cursor has no remote id")
                })?;
                self.pending = Some(Box::pin(async move {
                    executor
                        .connection_control(
                            "fetch_page",
                            json!({
                                "cursorId": cursor_id, "pageSize": 8,
                                "maxResponseBytes": 8 * 1024 * 1024,
                            }),
                        )
                        .await
                }));
            }
            let response = self
                .pending
                .as_mut()
                .expect("pending request exists")
                .await?;
            self.pending.take();
            self.cursor_id = Self::cursor_id(&response)?;
            self.has_more = response
                .get("hasMore")
                .and_then(JsonValue::as_bool)
                .unwrap_or(false);
            self.rows = Self::rows(&response)?;
            if self.rows.is_empty() && self.has_more {
                return Err(JdbcAgentConnection::protocol_error(
                    "Array cursor returned empty nonfinal page",
                ));
            }
        }
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        let value = self.current_value(column)?.into_druid()?;
        self.was_null.set(matches!(value, Value::Null));
        Ok(value)
    }

    fn object(&self, column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        let value = self.current_value(column)?.into_object()?;
        self.was_null.set(value.is_null());
        Ok(value)
    }

    fn row(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        Ok(if self.current.is_some() {
            self.row_number
        } else {
            0
        })
    }

    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.ensure_open()?;
        self.metadata
            .columns()
            .iter()
            .position(|column| column.label().eq_ignore_ascii_case(label))
            .map(|index| ColumnIndex::new(index + 1))
            .transpose()?
            .ok_or_else(|| RdbcError::InvalidArgument(format!("unknown Array column: {label}")))
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        Ok(Arc::clone(&self.metadata) as Arc<dyn ResultSetMetaData>)
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        Ok(self.was_null.get())
    }

    fn result_set_type(&self) -> Result<ResultSetType, RdbcError> {
        self.ensure_open()?;
        Ok(ResultSetType::ForwardOnly)
    }

    fn concurrency(&self) -> Result<ResultSetConcurrency, RdbcError> {
        self.ensure_open()?;
        Ok(ResultSetConcurrency::ReadOnly)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.closed {
            return Ok(());
        }
        if let Some(pending) = self.pending.as_mut() {
            let _ = pending.await;
        }
        self.pending.take();
        if let Some(cursor_id) = self.cursor_id.as_deref() {
            self.executor
                .connection_control("close_cursor", json!({"cursorId": cursor_id}))
                .await?;
            self.cursor_id.take();
        }
        self.rows.clear();
        self.current = None;
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.ensure_open().is_err())
    }
}

#[cfg(test)]
mod cancellation_tests {
    use super::JdbcAgentArrayResultSet;
    use crate::standard_adapter_executor::{owned_executor, AdapterExecutionCore};
    use crate::standard_query_result::StandardQueryResult;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::result_set::ColumnIndex;
    use druid_rdbc::types::{RdbcObject, Value};
    use serde_json::{json, Value as JsonValue};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::sync::Notify;

    struct PausedArrayPage {
        started: Arc<Notify>,
        released: Arc<Notify>,
        fetches: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AdapterExecutionCore for PausedArrayPage {
        async fn standard_connection_control_core(
            &mut self,
            operation: &str,
            _payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            match operation {
                "array_result_set" => Ok(json!({
                    "columns": [{"label": "INDEX", "jdbcType": -5}, {"label": "VALUE", "jdbcType": 4}],
                    "rows": [[{"kind": "int", "value": 1}, {"kind": "integer", "value": 7}]],
                    "cursorId": "cursor-1", "hasMore": true,
                })),
                "fetch_page" => {
                    self.fetches.fetch_add(1, Ordering::SeqCst);
                    self.started.notify_one();
                    self.released.notified().await;
                    Ok(json!({
                        "rows": [[{"kind": "int", "value": 2}, {"kind": "integer", "value": 9}]],
                        "cursorId": null, "hasMore": false,
                    }))
                }
                "close_cursor" => Ok(json!({})),
                _ => Err(RdbcError::InvalidArgument(operation.to_owned())),
            }
        }

        async fn standard_execute_update_core(
            &mut self,
            _sql: &str,
            _values: Vec<Value>,
        ) -> Result<i64, RdbcError> {
            unreachable!("Array test never executes SQL")
        }

        async fn standard_execute_query_core(
            &mut self,
            _sql: &str,
            _values: Vec<Value>,
        ) -> Result<StandardQueryResult, RdbcError> {
            unreachable!("Array test never executes SQL")
        }

        async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn cancelled_next_resumes_the_same_pending_remote_page() {
        let started = Arc::new(Notify::new());
        let released = Arc::new(Notify::new());
        let fetches = Arc::new(AtomicUsize::new(0));
        let executor = owned_executor(
            PausedArrayPage {
                started: Arc::clone(&started),
                released: Arc::clone(&released),
                fetches: Arc::clone(&fetches),
            },
            None,
        );
        let mut cursor = JdbcAgentArrayResultSet::open(
            executor,
            Arc::new(AtomicBool::new(true)),
            "array-1",
            None,
        )
        .await
        .expect("open Array cursor");
        assert!(cursor.next().await.expect("first row"));
        tokio::select! {
            result = cursor.next() => panic!("page unexpectedly completed: {result:?}"),
            () = started.notified() => {},
        }
        released.notify_one();
        assert!(cursor.next().await.expect("resume pending page"));
        assert_eq!(
            cursor
                .object(ColumnIndex::new(2).expect("column"))
                .expect("value"),
            RdbcObject::Integer(9),
        );
        assert_eq!(fetches.load(Ordering::SeqCst), 1, "fetch is not replayed");
        assert!(!cursor.next().await.expect("end of cursor"));
        cursor.close().await.expect("close cursor");
    }
}
