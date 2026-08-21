//! 统计 Filter 全局上下文。
//!
//! 对应 Java：`com.alibaba.druid.filter.stat.StatFilterContext`。

use super::StatFilterContextListener;
use crate::core::DruidError;
use std::sync::{Arc, OnceLock, RwLock};

static GLOBAL_CONTEXT: OnceLock<StatFilterContext> = OnceLock::new();

/// 以 copy-on-write 列表语义管理统计监听器。
///
/// 写操作复制并替换 listener 列表。Java 事件循环通过下标反复调用
/// `size()/get(i)`，所以回调内的增删可能影响当前轮剩余分发；Rust 每轮只在读取
/// 当前下标时持有读锁，回调期间不持锁，保留这一可观察行为。
pub struct StatFilterContext {
    listeners: RwLock<Vec<Arc<dyn StatFilterContextListener>>>,
}

impl StatFilterContext {
    /// 创建空上下文。
    pub fn new() -> Self {
        Self {
            listeners: RwLock::new(Vec::new()),
        }
    }

    /// 返回进程级单例。
    ///
    /// 对应 Java：`StatFilterContext#getInstance()`。
    pub fn global() -> &'static Self {
        GLOBAL_CONTEXT.get_or_init(Self::new)
    }

    /// 添加 listener；与 Java 一样允许同一实例重复注册。
    pub fn add_context_listener(&self, listener: Arc<dyn StatFilterContextListener>) {
        let mut listeners = self
            .listeners
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut next = listeners.clone();
        next.push(listener);
        *listeners = next;
    }

    /// 删除第一个指向同一 listener 实例的条目。
    pub fn remove_context_listener(&self, listener: &Arc<dyn StatFilterContextListener>) -> bool {
        let mut listeners = self
            .listeners
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(index) = listeners
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, listener))
        else {
            return false;
        };
        let mut next = listeners.clone();
        next.remove(index);
        *listeners = next;
        true
    }

    /// 返回当前 listener 的共享快照。
    pub fn listeners(&self) -> Vec<Arc<dyn StatFilterContextListener>> {
        self.snapshot()
    }

    /// 分发更新行数。
    pub fn add_update_count(&self, update_count: i32) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.add_update_count(update_count))
    }

    /// 分发抓取行数。
    pub fn add_fetch_row_count(&self, fetch_row_count: i32) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.add_fetch_row_count(fetch_row_count))
    }

    /// 分发 SQL 执行前事件。
    pub fn execute_before(&self, sql: &str, in_transaction: bool) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.execute_before(sql, in_transaction))
    }

    /// 分发 SQL 执行后事件。
    pub fn execute_after(
        &self,
        sql: Option<&str>,
        nano_span: i64,
        error: Option<&DruidError>,
    ) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.execute_after(sql, nano_span, error))
    }

    /// 分发提交事件。
    pub fn commit(&self) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.commit())
    }

    /// 分发回滚事件。
    pub fn rollback(&self) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.rollback())
    }

    /// 分发池化连接打开事件。
    pub fn pool_connection_open(&self) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.pool_connect())
    }

    /// 分发池化连接关闭事件。
    pub fn pool_connection_close(&self, nanos: i64) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.pool_close(nanos))
    }

    /// 分发物理连接创建事件。
    pub fn physical_connection_connect(&self) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.physical_connection_connect())
    }

    /// 分发物理连接关闭事件。
    pub fn physical_connection_close(&self, nanos: i64) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.physical_connection_close(nanos))
    }

    /// 分发 `ResultSet` 打开事件。
    pub fn result_set_open(&self) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.result_set_open())
    }

    /// 分发 `ResultSet` 关闭事件。
    pub fn result_set_close(&self, nanos: i64) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.result_set_close(nanos))
    }

    /// 分发 Clob 打开事件。
    pub fn clob_open(&self) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.clob_open())
    }

    /// 分发 Blob 打开事件。
    pub fn blob_open(&self) -> Result<(), DruidError> {
        self.dispatch(|listener| listener.blob_open())
    }

    fn snapshot(&self) -> Vec<Arc<dyn StatFilterContextListener>> {
        self.listeners
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn dispatch(
        &self,
        mut callback: impl FnMut(&Arc<dyn StatFilterContextListener>) -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        let mut index = 0;
        loop {
            let listener = self
                .listeners
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(index)
                .cloned();
            let Some(listener) = listener else {
                return Ok(());
            };
            callback(&listener)?;
            index += 1;
        }
    }
}

impl Default for StatFilterContext {
    fn default() -> Self {
        Self::new()
    }
}
