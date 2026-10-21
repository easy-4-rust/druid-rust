
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
unusable: Arc<AtomicBool>,
cursor_shutdown: watch::Sender<bool>,
auto_commit: AtomicBool,
read_only: AtomicBool,
savepoint_sequence: AtomicU64,
backend_key: &'static str,
url: String,
runtime_trace: Option<RuntimeTraceEmitter>,
