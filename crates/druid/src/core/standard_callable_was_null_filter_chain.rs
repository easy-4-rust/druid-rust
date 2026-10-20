use super::{BeforeFilter, DruidError};
use std::sync::Arc;

/// 按注册顺序包围 CallableStatement 的最近一次 OUT NULL 状态读取。
/// 对应 Java: FilterChainImpl#callableStatement_wasNull。
pub struct StandardCallableWasNullFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    terminal: &'a mut dyn FnMut() -> Result<bool, DruidError>,
}

impl<'a> StandardCallableWasNullFilterChain<'a> {
    /// 构造调用链；参数为过滤器序列和延迟驱动读取，返回首项开始的调用链。
    pub(crate) fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        terminal: &'a mut dyn FnMut() -> Result<bool, DruidError>,
    ) -> Self {
        Self {
            filters,
            position: 0,
            terminal,
        }
    }

    /// 调用下一个过滤器或驱动；无参数，返回可被包围过滤器改写的 NULL 标志。
    /// 对应 Java: FilterChain#callableStatement_wasNull。
    pub fn callable_statement_was_null(&mut self) -> Result<bool, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.standard_callable_was_null(self)
        } else {
            (self.terminal)()
        }
    }
}
