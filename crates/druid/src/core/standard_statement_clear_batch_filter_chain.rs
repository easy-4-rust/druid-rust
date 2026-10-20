use super::{BeforeFilter, DruidError};
use druid_rdbc::statement::Statement;
use std::sync::Arc;

/// 按注册顺序包围标准语句批次清除，允许过滤器拒绝、短路及处理驱动结果。
/// 对应 Java: FilterChainImpl#statement_clearBatch。
pub struct StandardStatementClearBatchFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    statement: &'a mut dyn Statement,
}

impl<'a> StandardStatementClearBatchFilterChain<'a> {
    /// 构造批次清除链；参数为过滤器和底层标准语句，返回从首项开始的链。
    pub(crate) fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        statement: &'a mut dyn Statement,
    ) -> Self {
        Self {
            filters,
            position: 0,
            statement,
        }
    }

    /// 执行下个过滤器或底层清除；无参数，返回原过滤或驱动操作结果。
    /// 对应 Java: FilterChain#statement_clearBatch。
    pub fn statement_clear_batch(&mut self) -> Result<(), DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.standard_statement_clear_batch(self)
        } else {
            self.statement.clear_batch().map_err(DruidError::from)
        }
    }
}
