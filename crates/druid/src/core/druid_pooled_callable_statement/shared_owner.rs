use super::{CallableStatementResource, CallableStatementState};
use crate::core::logical_statement_owner::LogicalStatementOwner;
use crate::core::prepared_statement_cache_stats::PreparedStatementCacheStats;
use crate::core::prepared_statement_key::PreparedStatementKey;
use crate::core::prepared_statement_property_snapshot::PreparedStatementPropertySnapshot;
use crate::core::standard_prepared_statement_cache::StandardPreparedStatementCache;
use crate::core::DruidPooledStatement;
use druid_rdbc::statement::Statement;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// 创建语句与结果集共享的逻辑状态，防止句柄复制分裂参数和关闭资格。
/// 对应 Java: com.alibaba.druid.pool.DruidPooledCallableStatement。
pub(super) struct CallableStatementSharedOwner {
    pub(super) statement_base: DruidPooledStatement,
    pub(super) resource: Arc<CallableStatementResource>,
    pub(super) sql: String,
    pub(super) key: Mutex<Option<PreparedStatementKey>>,
    pub(super) cache: Mutex<Option<Arc<tokio::sync::Mutex<StandardPreparedStatementCache>>>>,
    pub(super) cache_stats: Arc<PreparedStatementCacheStats>,
    pub(super) cache_entry: bool,
    pub(super) cacheable: AtomicBool,
    pub(super) initial_properties: PreparedStatementPropertySnapshot,
    pub(super) state: Mutex<CallableStatementState>,
    pub(super) close_lock: tokio::sync::Mutex<()>,
}

impl LogicalStatementOwner for CallableStatementSharedOwner {
    fn statement_handle(self: Arc<Self>) -> Box<dyn Statement> {
        Box::new(super::DruidPooledCallableStatement {
            statement_base: self.statement_base.clone(),
            shared: self,
        })
    }
}
