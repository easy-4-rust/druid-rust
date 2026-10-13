use super::{
    ConnectionEvent, ConnectionEventListener, PooledConnectionSource, StatementEvent,
    StatementEventListener, StatementSource,
};
use crate::error::SqlException;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, RwLock};

/// 为标准 pooled owner 提供线程安全的连接与语句事件监听器注册和快照分发。
///
/// 对应 Java: `DruidConnectionHolder` 的 `CopyOnWriteArrayList` 监听器列表。
/// 回调在释放注册表锁后执行；每次通知只访问开始时捕获的监听器快照。
#[derive(Default)]
pub struct PooledConnectionEventRegistry {
    connection_listeners: RwLock<Vec<Arc<dyn ConnectionEventListener>>>,
    statement_listeners: RwLock<Vec<Arc<dyn StatementEventListener>>>,
}

impl PooledConnectionEventRegistry {
    /// 创建空的事件注册表。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加连接事件监听器；重复注册会保留为多个独立注册项。
    pub fn add_connection_event_listener(&self, listener: Arc<dyn ConnectionEventListener>) {
        self.connection_listeners
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(listener);
    }

    /// 按 Arc 对象身份移除首个匹配的连接监听器；返回是否移除了注册项。
    pub fn remove_connection_event_listener(
        &self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> bool {
        let mut listeners = self
            .connection_listeners
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(index) = listeners
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, listener))
        else {
            return false;
        };
        listeners.remove(index);
        true
    }

    /// 添加预编译语句事件监听器；重复注册会保留为多个独立注册项。
    pub fn add_statement_event_listener(&self, listener: Arc<dyn StatementEventListener>) {
        self.statement_listeners
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(listener);
    }

    /// 按 Arc 对象身份移除首个匹配的语句监听器；返回是否移除了注册项。
    pub fn remove_statement_event_listener(
        &self,
        listener: &Arc<dyn StatementEventListener>,
    ) -> bool {
        let mut listeners = self
            .statement_listeners
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(index) = listeners
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, listener))
        else {
            return false;
        };
        listeners.remove(index);
        true
    }

    /// 通知逻辑连接已关闭，返回发生可捕获 panic 的回调数。
    ///
    /// 参数是触发该逻辑连接的实际 pooled owner 共享句柄；panic=abort 进程无法恢复。
    pub fn notify_connection_closed(&self, source: PooledConnectionSource) -> usize {
        let event = ConnectionEvent::new(source);
        self.connection_snapshot()
            .into_iter()
            .filter(|listener| {
                catch_unwind(AssertUnwindSafe(|| listener.connection_closed(&event))).is_err()
            })
            .count()
    }

    /// 通知连接发生 SQL 错误，保留原 owner 与 SQL 异常的共享身份。
    ///
    /// 返回发生可捕获 panic 的回调数；调用方仍负责按错误分类决定连接处置。
    pub fn notify_connection_error(
        &self,
        source: PooledConnectionSource,
        exception: Arc<SqlException>,
    ) -> usize {
        let event = ConnectionEvent::with_sql_exception(source, Some(exception));
        self.connection_snapshot()
            .into_iter()
            .filter(|listener| {
                catch_unwind(AssertUnwindSafe(|| {
                    listener.connection_error_occurred(&event)
                }))
                .is_err()
            })
            .count()
    }

    /// 通知 PreparedStatement 已关闭，保留 owner 和原语句身份。
    pub fn notify_statement_closed(
        &self,
        source: PooledConnectionSource,
        statement: StatementSource,
    ) -> usize {
        let event = StatementEvent::new(source, Some(statement));
        self.statement_snapshot()
            .into_iter()
            .filter(|listener| {
                catch_unwind(AssertUnwindSafe(|| listener.statement_closed(&event))).is_err()
            })
            .count()
    }

    /// 通知 PreparedStatement 发生 SQL 错误，保留 owner、原语句和异常身份。
    pub fn notify_statement_error(
        &self,
        source: PooledConnectionSource,
        statement: StatementSource,
        exception: Arc<SqlException>,
    ) -> usize {
        let event = StatementEvent::with_sql_exception(source, Some(statement), Some(exception));
        self.statement_snapshot()
            .into_iter()
            .filter(|listener| {
                catch_unwind(AssertUnwindSafe(|| {
                    listener.statement_error_occurred(&event)
                }))
                .is_err()
            })
            .count()
    }

    fn connection_snapshot(&self) -> Vec<Arc<dyn ConnectionEventListener>> {
        self.connection_listeners
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn statement_snapshot(&self) -> Vec<Arc<dyn StatementEventListener>> {
        self.statement_listeners
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}
