//! 对应 Java 类：`com.alibaba.druid.stat.RdbcStatManager`。

use super::{
    DruidDataSourceStatManager, RdbcConnectionStat, RdbcResultSetStat, RdbcStatContext,
    RdbcStatementStat,
};
use dashmap::DashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ExecutionKey {
    Task(tokio::task::Id),
    Thread(u64),
}

fn execution_key() -> ExecutionKey {
    if let Some(id) = tokio::task::try_id() {
        return ExecutionKey::Task(id);
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::thread::current().id().hash(&mut hasher);
    ExecutionKey::Thread(hasher.finish())
}

/// RDBC 代理层的进程级统计管理器。
///
/// Java `ThreadLocal` 在 Rust 中映射为 Tokio task identity，非 runtime 调用回退
/// 当前 OS thread identity；同时保留全局 Connection/Statement/ResultSet
/// 统计、SQL ID 和 reset 顺序。
pub struct RdbcStatManager {
    sql_id_seed: AtomicU64,
    connection_stat: RdbcConnectionStat,
    result_set_stat: RdbcResultSetStat,
    statement_stat: RdbcStatementStat,
    reset_count: AtomicU64,
    contexts: DashMap<ExecutionKey, RdbcStatContext>,
}

impl RdbcStatManager {
    /// 返回进程级单例。
    #[must_use]
    pub fn global() -> &'static Self {
        static INSTANCE: OnceLock<RdbcStatManager> = OnceLock::new();
        INSTANCE.get_or_init(|| Self {
            sql_id_seed: AtomicU64::new(1_000),
            connection_stat: RdbcConnectionStat::new(),
            result_set_stat: RdbcResultSetStat::new(),
            statement_stat: RdbcStatementStat::new(),
            reset_count: AtomicU64::new(0),
            contexts: DashMap::new(),
        })
    }

    /// 生成 SQL ID。对应 Java `generateSqlId()` 的先递增后返回。
    #[must_use]
    pub fn generate_sql_id(&self) -> u64 {
        self.sql_id_seed.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// 返回全局 Connection 统计。
    #[must_use]
    pub fn connection_stat(&self) -> &RdbcConnectionStat {
        &self.connection_stat
    }

    /// 返回全局 Statement 统计。
    #[must_use]
    pub fn statement_stat(&self) -> &RdbcStatementStat {
        &self.statement_stat
    }

    /// 返回全局 `ResultSet` 统计。
    #[must_use]
    pub fn result_set_stat(&self) -> &RdbcResultSetStat {
        &self.result_set_stat
    }

    /// 返回当前 task/thread 的统计上下文快照。
    #[must_use]
    pub fn stat_context(&self) -> Option<RdbcStatContext> {
        self.contexts
            .get(&execution_key())
            .map(|context| context.clone())
    }

    /// 设置或清除当前 task/thread 的统计上下文。
    pub fn set_stat_context(&self, context: Option<RdbcStatContext>) {
        let key = execution_key();
        match context {
            Some(context) => {
                self.contexts.insert(key, context);
            }
            None => {
                self.contexts.remove(&key);
            }
        }
    }

    /// 创建一个空统计上下文，不自动绑定。
    #[must_use]
    pub fn create_stat_context(&self) -> RdbcStatContext {
        RdbcStatContext::new()
    }

    /// 重置代理层全局统计与所有已注册数据源的 RDBC 统计。
    ///
    /// Java 在执行任何子 reset 之前先增加 `resetCount`；Rust 保持该顺序，
    /// 单个数据源是否允许重置仍由其 `resetStatEnable` 决定。
    pub fn reset(&self) {
        self.reset_count.fetch_add(1, Ordering::AcqRel);
        self.connection_stat.reset();
        self.statement_stat.reset();
        self.result_set_stat.reset();
        for (_, data_source) in DruidDataSourceStatManager::global().instances() {
            data_source.reset_rdbc_stat();
        }
    }

    /// 返回完成的 reset 调用次数。
    #[must_use]
    pub fn reset_count(&self) -> u64 {
        self.reset_count.load(Ordering::Acquire)
    }
}
