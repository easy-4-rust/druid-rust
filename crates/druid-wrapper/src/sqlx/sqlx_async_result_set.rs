use super::sqlx_connection_actor::SqlxConnectionActor;
use super::sqlx_cursor_request::SqlxCursorRequest;
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType, StatementOptions,
};
use druid_rdbc::types::{Row, Value};
use std::cell::Cell;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinHandle;

/// 独占标准 SQLx 异步前向游标；当前行及一个待接收响应占用有界空间。
/// 对应 Java: java.sql.ResultSet。物理连接始终由同一 actor 的任务持有。
pub(crate) struct SqlxAsyncResultSet {
    actor: Arc<SqlxConnectionActor>,
    metadata: Option<Arc<ResultSetMetaDataSnapshot>>,
    requests: Option<mpsc::Sender<SqlxCursorRequest>>,
    pending: Option<oneshot::Receiver<Result<Option<Row>, RdbcError>>>,
    shutdown: watch::Sender<bool>,
    statement_stop: watch::Receiver<bool>,
    worker: Option<JoinHandle<()>>,
    current: Option<Row>,
    was_null: Cell<bool>,
    row_number: i32,
    finished: bool,
    closed: bool,
    fetch_size: i32,
    direction: FetchDirection,
    options: StatementOptions,
}

impl SqlxAsyncResultSet {
    /// 创建受监管游标；参数为同一连接 actor、SQL、绑定值和标准选项。
    /// 返回仅完成元数据准备的结果集；取消创建 future 会触发 Drop 清理。
    pub(crate) async fn open(
        actor: Arc<SqlxConnectionActor>,
        sql: String,
        values: Vec<Value>,
        options: StatementOptions,
        fetch_size: i32,
        statement_stop: watch::Receiver<bool>,
        interrupted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let (requests, receiver) = mpsc::channel(1);
        let (startup, ready) = oneshot::channel();
        let (shutdown, stop) = watch::channel(false);
        let worker = actor.spawn_cursor(
            sql,
            values,
            receiver,
            startup,
            stop,
            statement_stop.clone(),
            interrupted,
        );
        // 在第一个 await 之前建立拥有 worker 的守卫，避免创建被取消后泄漏连接锁。
        let mut cursor = Self {
            actor,
            metadata: None,
            requests: Some(requests),
            pending: None,
            shutdown,
            statement_stop,
            worker: Some(worker),
            current: None,
            was_null: Cell::new(false),
            row_number: 0,
            finished: false,
            closed: false,
            fetch_size,
            direction: FetchDirection::Forward,
            options,
        };
        let columns = ready.await.map_err(|_| RdbcError::ConnectionDiscarded)??;
        cursor.metadata = Some(Arc::new(ResultSetMetaDataSnapshot::new(columns)));
        Ok(Box::new(cursor))
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed
            || self.actor.is_closed()
            || *self.statement_stop.borrow()
            || self.statement_stop.has_changed().is_err()
        {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }
}

impl Drop for SqlxAsyncResultSet {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
        self.requests.take();
    }
}

druid_rdbc::impl_standard_wrapper!(SqlxAsyncResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for SqlxAsyncResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        if self.finished {
            return Ok(false);
        }
        if self.pending.is_none() {
            let (reply, pending) = oneshot::channel();
            self.requests
                .as_ref()
                .ok_or(RdbcError::ConnectionDiscarded)?
                .try_send(SqlxCursorRequest { reply })
                .map_err(|_| RdbcError::ConnectionDiscarded)?;
            // 请求接纳与保存响应之间没有 await；取消 next 不会遗失尚未读到的行。
            self.pending = Some(pending);
        }
        let result = self
            .pending
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .await
            .map_err(|_| RdbcError::ConnectionDiscarded);
        self.pending.take();
        self.was_null.set(false);
        self.current = None;
        match result.and_then(std::convert::identity) {
            Ok(Some(row)) => {
                self.row_number = self
                    .row_number
                    .checked_add(1)
                    .ok_or_else(|| RdbcError::InvalidArgument("row exceeds JDBC i32".to_owned()))?;
                self.current = Some(row);
                self.actor.task4_trace_result_next();
                Ok(true)
            }
            Ok(None) => {
                self.finished = true;
                Ok(false)
            }
            Err(error) => {
                self.finished = true;
                Err(error)
            }
        }
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.ensure_open()?;
        let row = self
            .current
            .as_ref()
            .ok_or_else(|| RdbcError::InvalidArgument("result set is not on a row".to_owned()))?;
        let value = row.get(column.get() - 1).cloned().ok_or_else(|| {
            RdbcError::InvalidArgument(format!("column {} is out of range", column.get()))
        })?;
        self.was_null.set(matches!(value, Value::Null));
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
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .columns()
            .iter()
            .position(|column| column.label().eq_ignore_ascii_case(label))
            .map(|index| ColumnIndex::new(index + 1))
            .transpose()?
            .ok_or_else(|| RdbcError::InvalidArgument(format!("unknown column: {label}")))
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.ensure_open()?;
        self.actor.task4_trace_result_set_metadata();
        let metadata = self
            .metadata
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?;
        Ok(Arc::clone(metadata) as Arc<dyn ResultSetMetaData>)
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
        Ok(self.options.concurrency())
    }
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        self.ensure_open()?;
        Ok(self.options.holdability())
    }
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        Ok(self.fetch_size)
    }
    fn set_fetch_size(&mut self, size: i32) -> Result<(), RdbcError> {
        self.ensure_open()?;
        if size < 0 {
            return Err(RdbcError::InvalidArgument(
                "fetch size must not be negative".to_owned(),
            ));
        }
        self.fetch_size = size;
        Ok(())
    }
    fn fetch_direction(&self) -> Result<FetchDirection, RdbcError> {
        self.ensure_open()?;
        Ok(self.direction)
    }
    fn set_fetch_direction(&mut self, direction: FetchDirection) -> Result<(), RdbcError> {
        self.ensure_open()?;
        // 对应 Java: ResultSet#setFetchDirection，前向游标只允许 FETCH_FORWARD。
        if direction != FetchDirection::Forward {
            return Err(RdbcError::feature_not_supported(
                "sqlx_async_fetch_direction",
            ));
        }
        self.direction = direction;
        Ok(())
    }
    fn warnings(&self) -> Result<Option<SqlWarning>, RdbcError> {
        self.ensure_open()?;
        Ok(None)
    }
    fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        self.current = None;
        self.pending.take();
        self.shutdown.send_replace(true);
        self.requests.take();
        let outcome = if let Some(worker) = self.worker.as_mut() {
            // 可变引用等待保留取消安全；完成后无论成功或失败都必须移走句柄。
            worker.await.map_err(|error| {
                RdbcError::InvalidArgument(format!("SQLx cursor worker failed: {error}"))
            })
        } else {
            Ok(())
        };
        self.worker.take();
        outcome
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.ensure_open().is_err())
    }
}

#[cfg(test)]
mod tests;
