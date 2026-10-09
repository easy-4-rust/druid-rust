use super::{
    PreparedStatementCacheStats, PreparedStatementKey, PreparedStatementResource,
    PreparedStatementState, StandardPreparedStatementCache,
};
use crate::core::logical_statement_owner::LogicalStatementOwner;
use crate::core::DruidPooledStatement;
use druid_rdbc::statement::Statement;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

/// 创建语句与结果集共享的逻辑状态，防止句柄复制分裂参数和关闭资格。
/// 对应 Java: com.alibaba.druid.pool.DruidPooledPreparedStatement。
pub(super) struct PreparedStatementSharedOwner {
    pub(super) statement_base: DruidPooledStatement,
    pub(super) resource: Arc<PreparedStatementResource>,
    pub(super) sql: String,
    pub(super) key: Mutex<Option<PreparedStatementKey>>,
    pub(super) cache: Mutex<Option<Arc<tokio::sync::Mutex<StandardPreparedStatementCache>>>>,
    pub(super) cache_stats: Arc<PreparedStatementCacheStats>,
    pub(super) cache_entry: bool,
    pub(super) cacheable: AtomicBool,
    /// 当前逻辑租约已有成功执行；cached raw 的上一租约结果不可见。
    pub(super) has_execution_result: AtomicBool,
    pub(super) initial_properties:
        crate::core::prepared_statement_property_snapshot::PreparedStatementPropertySnapshot,
    pub(super) state: Mutex<PreparedStatementState>,
    pub(super) close_lock: tokio::sync::Mutex<()>,
}

impl LogicalStatementOwner for PreparedStatementSharedOwner {
    fn statement_handle(self: Arc<Self>) -> Box<dyn Statement> {
        Box::new(super::DruidPooledPreparedStatement {
            statement_base: self.statement_base.clone(),
            shared: self,
        })
    }
}
