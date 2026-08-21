//! Druid 默认 Wrapper 实现。

use super::wrapper::Wrapper;
use std::any::Any;

/// 只识别并解包自身的默认 Wrapper。
///
/// 对应 Java: `com.alibaba.druid.pool.WrapperAdapter`。
/// Java 对 `null` 返回 `false`/`null`，对当前实例类型返回自身；这些行为由
/// [`Wrapper`] 的默认实现逐项保留。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WrapperAdapter;

impl WrapperAdapter {
    /// 创建默认 Wrapper。
    ///
    /// 返回值对应 Java `WrapperAdapter()`。
    pub fn new() -> Self {
        Self
    }
}

impl Wrapper for WrapperAdapter {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
