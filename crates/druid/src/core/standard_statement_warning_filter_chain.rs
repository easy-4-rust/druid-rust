use super::{BeforeFilter, DruidError, SqlWarning};
use druid_rdbc::statement::Statement;
use std::sync::Arc;

/// 在标准语句上按注册顺序分派警告读取与清除，保留过滤器的包围和短路能力。
/// 对应 Java: FilterChainImpl#statement_getWarnings/statement_clearWarnings。
pub struct StandardStatementWarningFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    statement: &'a mut dyn Statement,
}

impl<'a> StandardStatementWarningFilterChain<'a> {
    /// 从首个过滤器开始构造调用链；参数为过滤器序列和标准语句。
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

    /// 读取下一个过滤器或驱动警告；无参数，返回完整可改写警告链。
    pub fn statement_get_warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.standard_statement_get_warnings(self)
        } else {
            self.statement.warnings().map_err(DruidError::from)
        }
    }

    /// 清除下一个过滤器或驱动警告；无参数，返回原操作结果。
    pub fn statement_clear_warnings(&mut self) -> Result<(), DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.standard_statement_clear_warnings(self)
        } else {
            self.statement.clear_warnings().map_err(DruidError::from)
        }
    }
}
