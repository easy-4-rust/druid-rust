//! Owned single-writer executor for canonical SQLx standard handles.

use super::sqlx_connection_adapter::SqlxConnectionAdapter;
use super::sqlx_streaming_result_set::SqlxStreamingResultSet;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::TablesQuery;
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::types::Value;
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
    pub(crate) fn new(connection: SqlxConnectionAdapter) -> Arc<Self> {
        let auto_commit = connection.standard_auto_commit_core();
        Arc::new(Self {
            connection: Mutex::new(connection),
            closed: AtomicBool::new(false),
            auto_commit: AtomicBool::new(auto_commit),
        })
    }

    /// 执行 query，并让 returned ResultSet 继续持有同一 actor lease。
    pub(crate) async fn execute_query(
        self: &Arc<SqlxConnectionActor>,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        let (rows, labels) = connection.fetch_rows_with_labels(sql, values).await?;
        Ok(self.result_set_core(rows, labels))
    }

    fn result_set_core(
        self: &Arc<SqlxConnectionActor>,
        rows: Vec<druid_rdbc::types::Row>,
        labels: Vec<String>,
    ) -> Box<dyn ResultSet> {
        Box::new(SqlxStreamingResultSet::values(rows, labels).with_actor_lease(Arc::clone(self)))
    }

    /// 执行 update，复用 adapter 的唯一 driver-neutral SQL core。
    pub(crate) async fn execute_update(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<i64, RdbcError> {
        self.ensure_open_core()?;
        let mut connection = self.connection.lock().await;
        let result = connection.execute_update_core(sql, values).await?;
        i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64 count".to_owned())
        })
    }

    /// 返回 live database product identity。
    pub(crate) async fn database_product_name(&self) -> Result<String, RdbcError> {
        self.ensure_open_core()?;
        self.connection.lock().await.standard_product_name_core()
    }

    pub(crate) async fn parameter_count(&self, sql: &str) -> Result<usize, RdbcError> {
        self.ensure_open_core()?;
        self.connection
            .lock()
            .await
            .standard_parameter_count_core(sql)
            .await
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
        let labels = LABELS.iter().map(|label| (*label).to_owned()).collect();
        if query
            .catalog
            .as_deref()
            .is_some_and(|catalog| !matches!(catalog, "" | "main"))
            || query
                .schema_pattern
                .as_deref()
                .is_some_and(|schema| !matches!(schema, "" | "main"))
        {
            return Ok(self.result_set_core(Vec::new(), labels));
        }
        let allowed_types = match query.table_types {
            None => None,
            Some(types) if types.is_empty() => {
                return Ok(self.result_set_core(Vec::new(), labels));
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
        Ok(self.result_set_core(rows, labels))
    }

    /// 返回 actor 维护的 auto-commit snapshot。
    pub(crate) fn auto_commit(&self) -> bool {
        self.auto_commit.load(Ordering::Acquire)
    }

    /// Exactly-once 关闭 raw connection；其他 handles 仍可观察 closed state。
    pub(crate) async fn close(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut connection = self.connection.lock().await;
        connection.close_connection_core().await?;
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    /// 返回 actor 是否已经关闭。
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
