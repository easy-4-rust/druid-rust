//! 对应 Java 类：`com.alibaba.druid.pool.ha.selector.StickyDataSourceHolder`。

use crate::core::Pool;
use std::sync::Arc;

/// 保存 sticky 数据源和取得时刻。
#[derive(Clone)]
pub struct StickyDataSourceHolder {
    retrieving_time_millis: u64,
    data_source: Option<Arc<dyn Pool>>,
}

impl StickyDataSourceHolder {
    /// 创建空 holder；取得时间仍按 Java 构造时初始化。
    #[must_use]
    pub fn new() -> Self {
        Self {
            retrieving_time_millis: crate::dynamic::epoch_millis(),
            data_source: None,
        }
    }

    /// 使用数据源创建 holder。
    #[must_use]
    pub fn with_data_source(data_source: Option<Arc<dyn Pool>>) -> Self {
        Self {
            retrieving_time_millis: crate::dynamic::epoch_millis(),
            data_source,
        }
    }

    /// 返回 holder 是否同时具有正时间和数据源。
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.retrieving_time_millis > 0 && self.data_source.is_some()
    }

    /// 返回取得时间。
    #[must_use]
    pub fn retrieving_time_millis(&self) -> u64 {
        self.retrieving_time_millis
    }

    /// 设置取得时间。
    pub fn set_retrieving_time_millis(&mut self, value: u64) {
        self.retrieving_time_millis = value;
    }

    /// 返回数据源。
    #[must_use]
    pub fn data_source(&self) -> Option<&Arc<dyn Pool>> {
        self.data_source.as_ref()
    }

    /// 设置数据源。
    pub fn set_data_source(&mut self, data_source: Option<Arc<dyn Pool>>) {
        self.data_source = data_source;
    }
}

impl Default for StickyDataSourceHolder {
    fn default() -> Self {
        Self::new()
    }
}
