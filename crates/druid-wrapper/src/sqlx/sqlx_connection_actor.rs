//! Owned single-writer executor for canonical SQLx standard handles.

use super::sqlx_async_result_set::SqlxAsyncResultSet;
use super::sqlx_buffered_result_set::SqlxBufferedResultSet;
use super::sqlx_connection_adapter::SqlxConnectionAdapter;
use super::sqlx_cursor_request::SqlxCursorRequest;
use crate::standard_adapter_executor::AdapterUpdateResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::metadata::{ColumnDescriptor, ColumnNullability, TablesQuery};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::Value;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::sync::{mpsc, oneshot, watch};

/// 串行拥有一个 SQLx raw connection 的 canonical executor/lease。
///
/// Statement、PreparedStatement、ResultSet 与 live DatabaseMetaData 只克隆本 handle；
/// raw connection 始终由 actor 单点拥有，调用借用结束后仍保持有效。
pub(crate) struct SqlxConnectionActor {
    connection: Mutex<SqlxConnectionAdapter>,
    closed: AtomicBool,
    cursor_shutdown: watch::Sender<bool>,
    auto_commit: AtomicBool,
    read_only: AtomicBool,
    savepoint_sequence: AtomicU64,
    backend_key: &'static str,
    url: String,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl SqlxConnectionActor {
    async fn clear_sqlite_progress_control(
        &self,
        connection: &mut SqlxConnectionAdapter,
    ) -> Result<(), RdbcError> {
        match tokio::time::timeout(
            Duration::from_secs(3),
            connection.clear_sqlite_progress_deadline(),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                connection.discard_after_unconfirmed_interrupt();
                Err(error)
            }
            Err(_) => {
                connection.discard_after_unconfirmed_interrupt();
                Err(RdbcError::ConnectionDiscarded)
            }
        }
    }

    fn ensure_open_core(&self) -> Result<(), RdbcError> {
        if self.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    /// 把 active adapter 转换成 shared owned actor。
    pub(crate) fn new(mut connection: SqlxConnectionAdapter) -> Arc<Self> {
        let auto_commit = connection.standard_auto_commit_core();
        let read_only = connection.standard_read_only_core();
        let savepoint_sequence = connection.standard_savepoint_sequence_core();
        let backend_key = connection.backend_key();
        let url = connection.standard_url_core().to_owned();
        let runtime_trace = connection.take_runtime_trace();
        Arc::new(Self {
            connection: Mutex::new(connection),
            closed: AtomicBool::new(false),
            cursor_shutdown: watch::channel(false).0,
            auto_commit: AtomicBool::new(auto_commit),
            read_only: AtomicBool::new(read_only),
            savepoint_sequence: AtomicU64::new(savepoint_sequence),
            backend_key,
            url,
            runtime_trace,
        })
    }

    #[track_caller]
    pub(crate) fn task4_trace_statement_create(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_statement_create();
        }
    }

    #[track_caller]
    pub(crate) fn task4_trace_statement_update(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_statement_update();
        }
    }

    #[track_caller]
    pub(crate) fn task4_trace_prepared_create(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_prepared_create();
        }
    }

    #[track_caller]
    pub(crate) fn task4_trace_parameter_set(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_parameter_set();
        }
    }

    #[track_caller]
    pub(crate) fn task4_trace_prepared_query(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_prepared_query();
        }
    }

    #[track_caller]
    pub(crate) fn task4_trace_result_next(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_result_next();
        }
    }

    #[track_caller]
    pub(crate) fn task4_trace_result_set_metadata(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_result_set_metadata();
        }
    }

    #[track_caller]
    pub(crate) fn task4_trace_database_metadata(&self) {
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_database_metadata();
        }
    }

    /// 带物理 SQLite VM 中断的准备查询执行；超时后等待 worker 停止再释放连接锁。
    pub(crate) async fn execute_query_with_timeout(
        self: &Arc<SqlxConnectionActor>,
        sql: &str,
        values: Vec<Value>,
        seconds: i32,
        interrupted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open_core()?;
        let duration = (seconds > 0).then(|| Duration::from_secs(seconds as u64));
        let deadline = duration.map(|duration| Instant::now() + duration);
        let mut connection = self.connection.lock().await;
        connection
            .install_sqlite_progress_control(deadline, Arc::clone(&interrupted))
            .await?;
        let outcome = match duration {
            Some(duration) => {
                tokio::time::timeout(duration, connection.fetch_rows_with_labels(sql, values)).await
            }
            None => Ok(connection.fetch_rows_with_labels(sql, values).await),
        };
        let timed_out = duration.is_some_and(|_| {
            outcome.is_err()
                || (matches!(&outcome, Ok(Err(_)))
                    && deadline.is_some_and(|deadline| Instant::now() >= deadline))
        });
        // SQLite worker 可能在等待 future 超时后仍持有数据库锁。先等待 VM 中断，
        // 再清除 handler；若三秒内无法确认释放，禁止复用此物理连接。
        match tokio::time::timeout(
            Duration::from_secs(3),
            connection.clear_sqlite_progress_deadline(),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                connection.discard_after_unconfirmed_interrupt();
                return Err(error);
            }
            Err(_) => {
                connection.discard_after_unconfirmed_interrupt();
                return Err(RdbcError::ConnectionDiscarded);
            }
        }
        if timed_out {
            return Err(RdbcError::SqlException(Box::new(
                SqlException::new(
                    0,
                    Some("HYT00".to_owned()),
                    Some("statement query timed out".to_owned()),
                )
                .with_class_name("java.sql.SQLTimeoutException")
                .with_assignable_type("java.sql.SQLTransientException"),
            )));
        }
        let (rows, columns) = match outcome {
            Ok(Ok(result)) => result,
            Ok(Err(_error)) if interrupted.load(std::sync::atomic::Ordering::Acquire) => {
                return Err(super::sqlx_prepared_statement::cancelled_exception());
            }
            Ok(Err(error)) => return Err(error),
            Err(_) => unreachable!("timeout was returned before extracting query rows"),
        };
        Ok(self.result_set_core(rows, columns))
    }

    /// 通过同一 actor 的物理连接创建按需游标；返回元数据准备后的标准结果集。
    pub(crate) async fn execute_async_query(
        self: &Arc<Self>,
        sql: &str,
        values: Vec<Value>,
        options: StatementOptions,
        fetch_size: i32,
        statement_stop: watch::Receiver<bool>,
        interrupted: Arc<AtomicBool>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open_core()?;
        SqlxAsyncResultSet::open(
            Arc::clone(self),
            sql.to_owned(),
            values,
            options,
            fetch_size,
            statement_stop,
            interrupted,
        )
        .await
    }

    /// 启动受父连接及游标关闭信号监管的唯一 raw cursor 任务。
    pub(super) fn spawn_cursor(
        self: &Arc<Self>,
        sql: String,
        values: Vec<Value>,
        mut requests: mpsc::Receiver<SqlxCursorRequest>,
        startup: oneshot::Sender<Result<Vec<ColumnDescriptor>, RdbcError>>,
        mut stop: watch::Receiver<bool>,
        mut statement_stop: watch::Receiver<bool>,
        interrupted: Arc<AtomicBool>,
    ) -> tokio::task::JoinHandle<()> {
        let actor = Arc::clone(self);
        let mut parent_stop = self.cursor_shutdown.subscribe();
        tokio::spawn(async move {
            let mut startup = Some(startup);
            if *stop.borrow()
                || *parent_stop.borrow()
                || *statement_stop.borrow()
                || statement_stop.has_changed().is_err()
            {
                return;
            }
            let outcome = {
                let work = async {
                    let mut connection = actor.connection.lock().await;
                    actor.ensure_open_core()?;
                    connection
                        .install_sqlite_progress_control(None, Arc::clone(&interrupted))
                        .await?;
                    let outcome = connection
                        .serve_cursor_core(
                            &sql,
                            values,
                            &mut startup,
                            &mut requests,
                            &mut stop,
                            &mut parent_stop,
                            &mut statement_stop,
                            Arc::clone(&interrupted),
                        )
                        .await;
                    match tokio::time::timeout(
                        Duration::from_secs(3),
                        connection.clear_sqlite_progress_deadline(),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            connection.discard_after_unconfirmed_interrupt();
                            return Err(error);
                        }
                        Err(_) => {
                            connection.discard_after_unconfirmed_interrupt();
                            return Err(RdbcError::ConnectionDiscarded);
                        }
                    }
                    match outcome {
                        Err(_) if interrupted.load(Ordering::Acquire) => {
                            Err(super::sqlx_prepared_statement::cancelled_exception())
                        }
                        outcome => outcome,
                    }
                };
                work.await
            };
            if let (Err(error), Some(sender)) = (outcome, startup) {
                let _ = sender.send(Err(error));
            }
        })
    }

    pub(crate) fn result_set_core(
        self: &Arc<SqlxConnectionActor>,
        rows: Vec<druid_rdbc::types::Row>,
        columns: Vec<ColumnDescriptor>,
    ) -> Box<dyn ResultSet> {
        Box::new(SqlxBufferedResultSet::values(rows, columns).with_actor_lease(Arc::clone(self)))
    }

    /// 为 SQLite INSERT RETURNING 在同一连接锁中保留结果行及真实生成键。
    /// 纯查询或未插入任何行时返回空键集，不能泄露该连接的前次插入键。
    pub(crate) async fn execute_query_with_generated_key(
        self: &Arc<SqlxConnectionActor>,
        sql: &str,
        values: Vec<Value>,
        seconds: i32,
        interrupted: Arc<AtomicBool>,
    ) -> Result<(Box<dyn ResultSet>, Vec<i64>), RdbcError> {
        self.ensure_open_core()?;
        let plan = crate::sqlite_returning_key_plan::SqliteReturningKeyPlan::parse(sql)?;
        let mut connection = self.connection.lock().await;
        let duration = (seconds > 0).then(|| Duration::from_secs(seconds as u64));
        let deadline = duration.map(|duration| Instant::now() + duration);
        connection
            .install_sqlite_progress_control(deadline, Arc::clone(&interrupted))
            .await?;
        let execution = async {
            if let Some(plan) = plan {
                let (properties, _) = connection
                    .fetch_rows_with_labels(&plan.table_properties_sql(), Vec::new())
                    .await?;
                if properties.is_empty() {
                    // 表已删除时执行原语句，让驱动保留真实 SQL 异常类别与消息。
                    let (rows, columns) = connection.fetch_rows_with_labels(sql, values).await?;
                    return Ok((self.result_set_core(rows, columns), Vec::new()));
                }
                let (table_columns, _) = connection
                    .fetch_rows_with_labels(&plan.columns_sql(&properties)?, Vec::new())
                    .await?;
                let (indexes, _) = connection
                    .fetch_rows_with_labels(&plan.indexes_sql(&properties)?, Vec::new())
                    .await?;
                let sql = plan.sql(&properties, &table_columns, &indexes);
                let (mut rows, mut columns) =
                    connection.fetch_rows_with_labels(&sql, values).await?;
                let keys =
                    crate::sqlite_returning_key_plan::SqliteReturningKeyPlan::extract(&mut rows)?;
                columns.pop();
                Ok((self.result_set_core(rows, columns), keys))
            } else {
                let (rows, columns) = connection.fetch_rows_with_labels(sql, values).await?;
                Ok((self.result_set_core(rows, columns), Vec::new()))
            }
        };
        let outcome = match duration {
            Some(duration) => tokio::time::timeout(duration, execution).await,
            None => Ok(execution.await),
        };
        let timed_out = duration.is_some_and(|_| {
            outcome.is_err()
                || (matches!(&outcome, Ok(Err(_)))
                    && deadline.is_some_and(|deadline| Instant::now() >= deadline))
        });
        self.clear_sqlite_progress_control(&mut connection).await?;
        if timed_out {
            Err(Self::statement_timeout_error())
        } else if interrupted.load(Ordering::Acquire) && matches!(&outcome, Ok(Err(_))) {
            Err(super::sqlx_prepared_statement::cancelled_exception())
        } else {
            outcome.unwrap_or_else(|_| unreachable!("timeout handled before result mapping"))
        }
    }

    /// 执行 update，并保留 generated-key/row-count driver outcome。
    /// 参数为SQL及值；在单个连接锁内返回真实更新数与完整当前键集合。
    pub(crate) async fn execute_update_keys(
        &self,
        sql: &str,
        values: Vec<Value>,
        seconds: i32,
        interrupted: Arc<AtomicBool>,
    ) -> Result<(i64, Vec<i64>), RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        let duration = (seconds > 0).then(|| Duration::from_secs(seconds as u64));
        let deadline = duration.map(|duration| Instant::now() + duration);
        connection
            .install_sqlite_progress_control(deadline, Arc::clone(&interrupted))
            .await?;
        let execution = async {
            if self.backend_key == "sqlite" {
                if let Some(plan) =
                    crate::sqlite_returning_key_plan::SqliteReturningKeyPlan::parse_update(sql)?
                {
                    let (properties, _) = connection
                        .fetch_rows_with_labels(&plan.table_properties_sql(), Vec::new())
                        .await?;
                    if !properties.is_empty() {
                        let (columns, _) = connection
                            .fetch_rows_with_labels(&plan.columns_sql(&properties)?, Vec::new())
                            .await?;
                        let (indexes, _) = connection
                            .fetch_rows_with_labels(&plan.indexes_sql(&properties)?, Vec::new())
                            .await?;
                        let (rows, _) = connection
                            .fetch_rows_with_labels(
                                &plan.sql(&properties, &columns, &indexes),
                                values,
                            )
                            .await?;
                        let (changes, _) = connection
                            .fetch_rows_with_labels("SELECT changes()", Vec::new())
                            .await?;
                        return crate::sqlite_returning_key_plan::SqliteReturningKeyPlan::update_outcome(rows, &changes);
                    }
                }
            }
            let result = connection.execute_update_core(sql, values).await?;
            let count = i64::try_from(result.rows_affected).map_err(|_| {
                RdbcError::InvalidArgument("rows affected exceeds canonical i64".into())
            })?;
            Ok((count, result.last_insert_id.into_iter().collect()))
        };
        let outcome = match duration {
            Some(duration) => tokio::time::timeout(duration, execution).await,
            None => Ok(execution.await),
        };
        let timed_out = duration.is_some_and(|_| {
            outcome.is_err()
                || (matches!(&outcome, Ok(Err(_)))
                    && deadline.is_some_and(|deadline| Instant::now() >= deadline))
        });
        self.clear_sqlite_progress_control(&mut connection).await?;
        if timed_out {
            Err(Self::statement_timeout_error())
        } else if interrupted.load(Ordering::Acquire) && matches!(&outcome, Ok(Err(_))) {
            Err(super::sqlx_prepared_statement::cancelled_exception())
        } else {
            outcome.unwrap_or_else(|_| unreachable!("timeout handled before result mapping"))
        }
    }

    /// 执行 update，并保留 generated-key/row-count driver outcome。
    pub(crate) async fn execute_update_result(
        &self,
        sql: &str,
        values: Vec<Value>,
        seconds: i32,
        interrupted: Arc<AtomicBool>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        let duration = (seconds > 0).then(|| Duration::from_secs(seconds as u64));
        let deadline = duration.map(|duration| Instant::now() + duration);
        connection
            .install_sqlite_progress_control(deadline, Arc::clone(&interrupted))
            .await?;
        let execution = connection.execute_update_core(sql, values);
        let outcome = match duration {
            Some(duration) => tokio::time::timeout(duration, execution).await,
            None => Ok(execution.await),
        };
        let timed_out = duration.is_some_and(|_| {
            outcome.is_err()
                || (matches!(&outcome, Ok(Err(_)))
                    && deadline.is_some_and(|deadline| Instant::now() >= deadline))
        });
        self.clear_sqlite_progress_control(&mut connection).await?;
        if timed_out {
            Err(Self::statement_timeout_error())
        } else if interrupted.load(Ordering::Acquire) && matches!(&outcome, Ok(Err(_))) {
            Err(super::sqlx_prepared_statement::cancelled_exception())
        } else {
            outcome.unwrap_or_else(|_| unreachable!("timeout handled before result mapping"))
        }
    }

    fn statement_timeout_error() -> RdbcError {
        RdbcError::SqlException(Box::new(
            SqlException::new(
                0,
                Some("HYT00".to_owned()),
                Some("statement query timed out".to_owned()),
            )
            .with_class_name("java.sql.SQLTimeoutException")
            .with_assignable_type("java.sql.SQLTransientException"),
        ))
    }

    /// 执行 eager query，仅返回 canonical rows。
    pub(crate) async fn fetch_rows(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<Vec<druid_rdbc::types::Row>, RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .fetch_rows_with_labels(sql, values)
            .await
            .map(|(rows, _)| rows)
    }

    /// 返回 live database product identity。
    pub(crate) async fn database_product_name(&self) -> Result<String, RdbcError> {
        self.ensure_open_core()?;
        self.connection.lock().await.standard_product_name_core()
    }

    /// 返回 live database version。
    pub(crate) async fn database_product_version(&self) -> Result<String, RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .standard_product_version_core()
            .await
    }

    /// 返回 authenticated database user；driver 不暴露时保持 None/unsupported。
    pub(crate) async fn database_user_name(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open_core()?;
        self.connection.lock().await.standard_user_name_core().await
    }

    /// 返回 driver-backed read-only metadata state。
    pub(crate) async fn database_read_only(&self) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .standard_read_only_metadata_core()
            .await
    }

    pub(crate) async fn parameter_count(&self, sql: &str) -> Result<usize, RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .standard_parameter_count_core(sql)
            .await
    }

    /// 返回 driver describe 生成的 immutable result metadata columns。
    pub(crate) async fn describe_columns(
        &self,
        sql: &str,
    ) -> Result<Vec<ColumnDescriptor>, RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .describe_columns_core(sql)
            .await
    }

    /// 在判断 execute 结果种类时也使用同一取消代次与查询期限。
    pub(crate) async fn statement_returns_rows_with_control(
        &self,
        sql: &str,
        seconds: i32,
        interrupted: Arc<AtomicBool>,
    ) -> Result<bool, RdbcError> {
        self.ensure_open_core()?;
        let duration = (seconds > 0).then(|| Duration::from_secs(seconds as u64));
        let deadline = duration.map(|duration| Instant::now() + duration);
        let mut connection = self.connection.lock().await;
        connection
            .install_sqlite_progress_control(deadline, Arc::clone(&interrupted))
            .await?;
        let describe = connection.describe_columns_core(sql);
        let outcome = match duration {
            Some(duration) => tokio::time::timeout(duration, describe).await,
            None => Ok(describe.await),
        };
        let timed_out = duration.is_some_and(|_| {
            outcome.is_err()
                || (matches!(&outcome, Ok(Err(_)))
                    && deadline.is_some_and(|deadline| Instant::now() >= deadline))
        });
        self.clear_sqlite_progress_control(&mut connection).await?;
        if timed_out {
            return Err(RdbcError::SqlException(Box::new(
                SqlException::new(
                    0,
                    Some("HYT00".to_owned()),
                    Some("statement query timed out".to_owned()),
                )
                .with_class_name("java.sql.SQLTimeoutException")
                .with_assignable_type("java.sql.SQLTransientException"),
            )));
        }
        let columns = match outcome {
            Ok(Ok(columns)) => columns,
            Ok(Err(_)) if interrupted.load(Ordering::Acquire) => {
                return Err(super::sqlx_prepared_statement::cancelled_exception());
            }
            Ok(Err(error)) => return Err(error),
            Err(_) => unreachable!("timeout was returned before extracting describe result"),
        };
        Ok(!columns.is_empty())
    }

    /// 执行 SQLite live tables query，并返回继续持有 actor lease 的 ResultSet。
    pub(crate) async fn tables(
        self: &Arc<SqlxConnectionActor>,
        query: TablesQuery,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open_core()?;
        const LABELS: &[&str] = &[
            "TABLE_CAT",
            "TABLE_SCHEM",
            "TABLE_NAME",
            "TABLE_TYPE",
            "REMARKS",
            "TYPE_CAT",
            "TYPE_SCHEM",
            "TYPE_NAME",
            "SELF_REFERENCING_COL_NAME",
            "REF_GENERATION",
        ];
        let columns = LABELS
            .iter()
            .map(|label| ColumnDescriptor::new(*label, "VARCHAR", ColumnNullability::Unknown))
            .collect::<Vec<_>>();
        if query
            .catalog
            .as_deref()
            .is_some_and(|catalog| !std::matches!(catalog, "" | "main"))
            || query
                .schema_pattern
                .as_deref()
                .is_some_and(|schema| !std::matches!(schema, "" | "main"))
        {
            return Ok(self.result_set_core(Vec::new(), columns));
        }
        let allowed_types = match query.table_types {
            None => None,
            Some(types) if types.is_empty() => {
                return Ok(self.result_set_core(Vec::new(), columns));
            }
            Some(types) => Some(
                types
                    .into_iter()
                    .map(|value| value.to_ascii_uppercase())
                    .collect::<Vec<_>>(),
            ),
        };
        let sql = concat!(
            "select null as TABLE_CAT, null as TABLE_SCHEM, name as TABLE_NAME, ",
            "upper(type) as TABLE_TYPE, null as REMARKS, null as TYPE_CAT, null as TYPE_SCHEM, ",
            "null as TYPE_NAME, null as SELF_REFERENCING_COL_NAME, null as REF_GENERATION ",
            "from sqlite_master where type in ('table', 'view') and name like ? order by name"
        );
        let pattern = query.table_name_pattern.unwrap_or_else(|| "%".to_owned());
        let mut connection = self.connection.lock().await;
        if connection.backend_key() != "sqlite" {
            return Err(RdbcError::UnsupportedOperation {
                operation: "sqlx_database_metadata_tables_backend",
            });
        }
        let (mut rows, _) = connection
            .fetch_rows_with_labels(sql, vec![Value::String(pattern)])
            .await?;
        if let Some(allowed_types) = allowed_types {
            rows.retain(|row| {
                row.values
                    .get(3)
                    .and_then(|value| match value {
                        Value::String(value) => Some(value),
                        _ => None,
                    })
                    .is_some_and(|value| allowed_types.iter().any(|allowed| allowed == value))
            });
        }
        drop(connection);
        Ok(self.result_set_core(rows, columns))
    }

    /// 返回 actor 维护的 auto-commit snapshot。
    pub(crate) fn auto_commit(&self) -> bool {
        self.auto_commit.load(Ordering::Acquire)
    }

    /// 验证 actor 独占的 raw SQLx 连接仍可用。
    pub(crate) async fn ping(&self) -> Result<(), RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        self.ensure_open_core()?;
        connection.ping_raw_core().await
    }

    /// 切换 actor 独占连接的 auto-commit 状态。
    pub(crate) async fn set_auto_commit(&self, enabled: bool) -> Result<(), RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        self.ensure_open_core()?;
        match (connection.standard_auto_commit_core(), enabled) {
            (true, false) => connection.begin_raw_core().await?,
            (false, true) => connection.commit_raw_core().await?,
            _ => return Ok(()),
        }
        self.auto_commit.store(enabled, Ordering::Release);
        Ok(())
    }

    /// 提交 actor 独占连接的当前 transaction。
    pub(crate) async fn commit(&self) -> Result<(), RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        self.ensure_open_core()?;
        connection.commit_raw_core().await?;
        self.auto_commit.store(true, Ordering::Release);
        Ok(())
    }

    /// 回滚 actor 独占连接的当前 transaction。
    pub(crate) async fn rollback(&self) -> Result<(), RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        self.ensure_open_core()?;
        connection.rollback_raw_core().await?;
        self.auto_commit.store(true, Ordering::Release);
        Ok(())
    }

    /// 执行 savepoint/control SQL。
    pub(crate) async fn execute_control_statement(&self, sql: &str) -> Result<(), RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .execute_control_statement(sql)
            .await
    }

    /// 分配下一个 connection-local savepoint ID。
    pub(crate) fn next_savepoint_id(&self) -> u64 {
        self.savepoint_sequence.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// 返回 read-only snapshot。
    pub(crate) fn read_only(&self) -> bool {
        self.read_only.load(Ordering::Acquire)
    }

    /// 设置 raw SQLx session 的 read-only 状态。
    pub(crate) async fn set_read_only(&self, read_only: bool) -> Result<(), RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .set_read_only_raw_core(read_only)
            .await?;
        self.read_only.store(read_only, Ordering::Release);
        Ok(())
    }

    /// 返回 backend identity。
    pub(crate) const fn backend_key(&self) -> &'static str {
        self.backend_key
    }

    /// 返回配置 URL；metadata 层负责避免把它写入日志。
    pub(crate) fn database_url(&self) -> &str {
        &self.url
    }

    /// SQLx 不暴露 JDBC warning chain；存活 actor 返回空链。
    pub(crate) fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.ensure_open_core()?;
        Ok(None)
    }

    /// SQLx 不保存可清理的 warning state；存活 actor no-op 成功。
    pub(crate) fn clear_warnings(&self) -> Result<(), RdbcError> {
        self.ensure_open_core()
    }

    /// Exactly-once 关闭 raw connection；其他 handles 仍可观察 closed state。
    pub(crate) async fn close(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        // 先通知持有连接锁的游标退出，再等待锁；避免父关闭与空闲游标互等。
        self.cursor_shutdown.send_replace(true);
        let mut connection = self.connection.lock().await;
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        connection.close_connection_core().await?;
        if let Some(trace) = &self.runtime_trace {
            trace.task4_trace_connection_close();
        }
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    /// 返回 actor 是否已经关闭。
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire) || *self.cursor_shutdown.borrow()
    }
}
