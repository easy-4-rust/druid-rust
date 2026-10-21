//! 对外池化连接。

use super::connection_event_listener::ConnectionEventListener;
use super::connection_recycle_disposition::ConnectionRecycleDisposition;
use super::database_meta_data_proxy_impl::DatabaseMetaDataProxyImpl;
use super::druid_connection_holder::DruidConnectionHolder;
use super::druid_execution_filter_policy::DruidExecutionFilterPolicy;
use super::druid_pooled_callable_statement::DruidPooledCallableStatement;
use super::druid_pooled_prepared_statement::DruidPooledPreparedStatement;
use super::druid_pooled_statement::DruidPooledStatement;
use super::error::{DruidError, RdbcError};
use super::exception_sorter::ExceptionSorter;
use super::fatal_error_handler::FatalErrorHandler;
use super::filter::ConnectionEvent;
use super::filter_chain::FilterChain;
use super::owner_disposition_control::OwnerDispositionControl;
use super::prepared_statement_key::PreparedStatementKey;
use super::prepared_statement_method_type::PreparedStatementMethodType;
use super::proxy_attributes::{ProxyAttributeValue, ProxyAttributes};
use super::sql_warning::SqlWarning;
use super::standard_logical_policy::StandardLogicalPolicy;
use super::statement_event_listener::StatementEventListener;
use super::transaction_info::TransactionInfo;
use crate::stats::StatsCollector;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

mod inherent_a;
mod inherent_b;

mod standard_connection;

mod standard_connection_helpers;

/// Native pool 回收完整 holder 的一次性回调。
pub type DruidConnectionReturnCallback =
    Box<dyn FnOnce(DruidConnectionHolder, ConnectionRecycleDisposition) -> bool + Send>;

type NativeExplicitReturnCallback = Box<
    dyn FnOnce(DruidConnectionHolder, ConnectionRecycleDisposition) -> Option<DruidConnectionHolder>
        + Send,
>;

#[cfg(test)]
mod result_set_error_lease_tests;

/// Synchronous one-shot discard notification, separate from the later holder return.
pub type DruidConnectionDispositionObserver =
    Box<dyn FnOnce(ConnectionRecycleDisposition) + Send + 'static>;

/// 外部连接池 canonical standard lease 的一次性归还回调。
pub type StandardConnectionReturnCallback = Box<
    dyn FnOnce(Box<dyn druid_rdbc::connection::Connection>, ConnectionRecycleDisposition) + Send,
>;

/// 对外池化连接。
///
/// 对应 Java: `com.alibaba.druid.pool.DruidPooledConnection`。
/// 该对象拥有一次连接租约，对外暴露连接语义；底层只依赖
/// canonical [`druid_rdbc::connection::Connection`]。显式关闭和 `Drop` 都通过同一条
/// 回收路径，并由 take-once 回调保证 driver connection 最多归还一次。显式异步关闭执行 Java
/// `DruidDataSource#recycle` 的事务回滚、状态复位与 return validation；
/// Drop 只复用无需异步复位的干净连接。
pub struct DruidPooledConnection {
    holder: Option<DruidConnectionHolder>,
    id: u64,
    data_source: String,
    filter_chain: Option<Arc<FilterChain>>,
    keep_underlying_transaction_isolation: bool,
    standard_test_on_return: bool,
    native_return_validation: Option<Arc<crate::pool::PoolInner>>,
    /// 显式归还的完成等待端；取消 future 后仍保留，支持再次关闭继续等待。
    pending_return_completion: Option<tokio::sync::oneshot::Receiver<()>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    fatal_error_handler: Option<Arc<dyn FatalErrorHandler>>,
    return_connection: Option<DruidConnectionReturnCallback>,
    native_explicit_return: Option<NativeExplicitReturnCallback>,
    owner_disposition: Arc<OwnerDispositionControl>,
    lease_active: Arc<AtomicBool>,
    execution_running: Arc<AtomicBool>,
    standard_policy: Option<Arc<StandardLogicalPolicy>>,
    /// 由标准 pooled owner 管理时，逻辑关闭只结束租约并保留 holder。
    owner_managed: bool,
    borrowed_at: Instant,
    recycled: bool,
    connection_closed_notified: bool,
    stats_collector: Option<Arc<StatsCollector>>,
    transaction_info: Option<Arc<TransactionInfo>>,
    query_timeout: i32,
    transaction_query_timeout: i32,
    statement_id_seed: Arc<AtomicU64>,
    result_set_id_seed: Arc<AtomicU64>,
    metadata_id_seed: Arc<AtomicU64>,
    connection_properties: Arc<HashMap<String, String>>,
    connected_time_millis: u64,
    close_count: u64,
    last_validate_time_millis: u64,
    attributes: ProxyAttributes,
    /// 当前租约首次修改前的客户端信息；缺少快照时禁止污染连接复用。
    initial_client_info: Option<HashMap<String, String>>,
    client_info_touched: bool,
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for DruidPooledConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledConnection")
            .field("id", &self.id)
            .field("data_source", &self.data_source)
            .field(
                "holder",
                &self
                    .holder
                    .as_ref()
                    .is_some_and(DruidConnectionHolder::has_standard_connection),
            )
            .field(
                "has_connection",
                &self
                    .holder
                    .as_ref()
                    .is_some_and(DruidConnectionHolder::has_standard_connection),
            )
            .field("filter_chain", &self.filter_chain.is_some())
            .field(
                "keep_underlying_transaction_isolation",
                &self.keep_underlying_transaction_isolation,
            )
            .field("exception_sorter", &self.exception_sorter.is_some())
            .field("fatal_error_handler", &self.fatal_error_handler.is_some())
            .field("return_connection", &self.return_connection.is_some())
            .field(
                "owner_disposition_signaled",
                &self.owner_disposition.is_signaled(),
            )
            .field("lease_active", &self.lease_active.load(Ordering::Acquire))
            .field(
                "execution_running",
                &self.execution_running.load(Ordering::Acquire),
            )
            .field("borrowed_at", &self.borrowed_at)
            .field("recycled", &self.recycled)
            .field(
                "connection_closed_notified",
                &self.connection_closed_notified,
            )
            .finish()
    }
}

druid_rdbc::impl_standard_wrapper!(DruidPooledConnection => Connection);

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

impl Drop for DruidPooledConnection {
    fn drop(&mut self) {
        if self.recycled {
            return;
        }
        self.notify_connection_closed();
        let disposition = self.drop_disposition();
        self.recycle_once(disposition);
    }
}
