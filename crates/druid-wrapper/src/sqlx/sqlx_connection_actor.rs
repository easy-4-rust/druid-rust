//! Owned single-writer executor for canonical SQLx standard handles.

use super::sqlx_connection_adapter::SqlxConnectionAdapter;
use super::sqlx_streaming_result_set::SqlxStreamingResultSet;
use crate::standard_adapter_executor::AdapterUpdateResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ColumnDescriptor, ColumnNullability, TablesQuery};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::types::Value;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 串行拥有一个 SQLx raw connection 的 canonical executor/lease。
///
/// Statement、PreparedStatement、ResultSet 与 live DatabaseMetaData 只克隆本 handle；
/// raw connection 始终由 actor 单点拥有，调用借用结束后仍保持有效。
pub(crate) struct SqlxConnectionActor {
    connection: Mutex<SqlxConnectionAdapter>,
    closed: AtomicBool,
    auto_commit: AtomicBool,
    read_only: AtomicBool,
    savepoint_sequence: AtomicU64,
    backend_key: &'static str,
    url: String,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl SqlxConnectionActor {
    fn ensure_open_core(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
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

    /// 执行 query，并让 returned ResultSet 继续持有同一 actor lease。
    pub(crate) async fn execute_query(
        self: &Arc<SqlxConnectionActor>,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        let (rows, columns) = connection.fetch_rows_with_labels(sql, values).await?;
        Ok(self.result_set_core(rows, columns))
    }

    pub(crate) fn result_set_core(
        self: &Arc<SqlxConnectionActor>,
        rows: Vec<druid_rdbc::types::Row>,
        columns: Vec<ColumnDescriptor>,
    ) -> Box<dyn ResultSet> {
        Box::new(SqlxStreamingResultSet::values(rows, columns).with_actor_lease(Arc::clone(self)))
    }

    /// 为 SQLite INSERT RETURNING 在同一连接锁中保留结果行及真实生成键。
    /// 纯查询或未插入任何行时返回空键集，不能泄露该连接的前次插入键。
    pub(crate) async fn execute_query_with_generated_key(
        self: &Arc<SqlxConnectionActor>,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<(Box<dyn ResultSet>, Vec<i64>), RdbcError> {
        self.ensure_open_core()?;
        let plan = crate::sqlite_returning_key_plan::SqliteReturningKeyPlan::parse(sql)?;
        let mut connection = self.connection.lock().await;
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
            let (mut rows, mut columns) = connection.fetch_rows_with_labels(&sql, values).await?;
            let keys =
                crate::sqlite_returning_key_plan::SqliteReturningKeyPlan::extract(&mut rows)?;
            columns.pop();
            Ok((self.result_set_core(rows, columns), keys))
        } else {
            let (rows, columns) = connection.fetch_rows_with_labels(sql, values).await?;
            Ok((self.result_set_core(rows, columns), Vec::new()))
        }
    }

    /// 执行 update，并保留 generated-key/row-count driver outcome。
    /// 参数为SQL及值；在单个连接锁内返回真实更新数与完整当前键集合。
    pub(crate) async fn execute_update_keys(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<(i64, Vec<i64>), RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
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
                        .fetch_rows_with_labels(&plan.sql(&properties, &columns, &indexes), values)
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
    }

    /// 执行 update，并保留 generated-key/row-count driver outcome。
    pub(crate) async fn execute_update_result(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .execute_update_core(sql, values)
            .await
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

    /// 返回 SQLx describe 是否报告 result columns；不检查 SQL 文本前缀。
    pub(crate) async fn statement_returns_rows(&self, sql: &str) -> Result<bool, RdbcError> {
        self.describe_columns(sql)
            .await
            .map(|columns| !columns.is_empty())
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
        self.closed.load(Ordering::Acquire)
    }
}
