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
        let mut connection = self.connection.lock().await;
        let (rows, labels) = connection.fetch_rows_with_labels(sql, values).await?;
        Ok(Box::new(
            SqlxStreamingResultSet::values(rows, labels).with_actor_lease(Arc::clone(self)),
        ))
    }

    /// 执行 update，复用 adapter 的唯一 driver-neutral SQL core。
    pub(crate) async fn execute_update(
        &self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<i64, RdbcError> {
        let mut connection = self.connection.lock().await;
        let result = connection.execute_update_core(sql, values).await?;
        i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64 count".to_owned())
        })
    }

    /// 返回 live database product identity。
    pub(crate) async fn database_product_name(&self) -> Result<String, RdbcError> {
        self.connection.lock().await.standard_product_name_core()
    }

    /// 执行 SQLite live tables query，并返回继续持有 actor lease 的 ResultSet。
    pub(crate) async fn tables(
        self: &Arc<SqlxConnectionActor>,
        query: TablesQuery,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        let sql = concat!(
            "select null as TABLE_CAT, null as TABLE_SCHEM, name as TABLE_NAME, ",
            "upper(type) as TABLE_TYPE, null as REMARKS, null as TYPE_CAT, null as TYPE_SCHEM, ",
            "null as TYPE_NAME, null as SELF_REFERENCING_COL_NAME, null as REF_GENERATION ",
            "from sqlite_master where type in ('table', 'view') and name like ? order by name"
        );
        let pattern = query.table_name_pattern.unwrap_or_else(|| "%".to_owned());
        self.execute_query(sql, vec![Value::String(pattern)]).await
    }

    /// 返回 actor 维护的 auto-commit snapshot。
    pub(crate) fn auto_commit(&self) -> bool {
        self.auto_commit.load(Ordering::Acquire)
    }

    /// Exactly-once 关闭 raw connection；其他 handles 仍可观察 closed state。
    pub(crate) async fn close(&self) -> Result<(), RdbcError> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let mut connection = self.connection.lock().await;
        connection.standard_close_core();
        Ok(())
    }

    /// 返回 actor 是否已经关闭。
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
