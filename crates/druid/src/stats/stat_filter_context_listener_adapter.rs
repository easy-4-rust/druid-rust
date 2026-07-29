//! 统计 Filter 上下文监听器空实现。
//!
//! 对应 Java：
//! `com.alibaba.druid.filter.stat.StatFilterContextListenerAdapter`。

use super::StatFilterContextListener;
use crate::core::DruidError;

/// 为不需要处理任何统计事件的调用方提供完整空实现。
///
/// Java 调用方通过继承该类只覆盖关心的方法；Rust 调用方直接实现
/// [`StatFilterContextListener`]。本对象保留 Java 实例本身“所有事件均成功且
/// 无副作用”的可观察行为。
#[derive(Debug, Clone, Copy, Default)]
pub struct StatFilterContextListenerAdapter;

impl StatFilterContextListenerAdapter {
    /// 创建空监听器适配器。
    pub const fn new() -> Self {
        Self
    }
}

impl StatFilterContextListener for StatFilterContextListenerAdapter {
    fn add_update_count(&self, _update_count: i32) -> Result<(), DruidError> {
        Ok(())
    }

    fn add_fetch_row_count(&self, _fetch_row_count: i32) -> Result<(), DruidError> {
        Ok(())
    }

    fn execute_before(&self, _sql: &str, _in_transaction: bool) -> Result<(), DruidError> {
        Ok(())
    }

    fn execute_after(
        &self,
        _sql: Option<&str>,
        _nano_span: i64,
        _error: Option<&DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    fn commit(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn rollback(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn pool_connect(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn pool_close(&self, _nanos: i64) -> Result<(), DruidError> {
        Ok(())
    }

    fn physical_connection_connect(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn physical_connection_close(&self, _nanos: i64) -> Result<(), DruidError> {
        Ok(())
    }

    fn result_set_open(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn result_set_close(&self, _nanos: i64) -> Result<(), DruidError> {
        Ok(())
    }

    fn clob_open(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn blob_open(&self) -> Result<(), DruidError> {
        Ok(())
    }
}
