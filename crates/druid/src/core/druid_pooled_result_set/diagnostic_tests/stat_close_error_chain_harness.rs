use crate::core::{
    DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext, StandardResultSetOpenContext,
};
use druid_rdbc::metadata::ResultSetMetaDataSnapshot;
use druid_rdbc::result_set::ResultSet;
use std::sync::Arc;

/// 将原开放和关闭入口委托到同一实际标准过滤链。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl#resultSet_close。
pub(super) struct StatCloseErrorChainHarness {
    chain: FilterChain,
}

impl StatCloseErrorChainHarness {
    /// 创建空链；无参数，返回实际标准链的测试适配。
    pub(super) fn new() -> Self {
        Self {
            chain: FilterChain::new(),
        }
    }

    /// 添加原统计过滤器；参数为共享过滤器，无返回值。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.chain.add_result_set(filter);
    }

    /// 调用实际开放后置链；参数为原上下文，返回原错误。
    pub(super) fn result_set_open_after(
        &self,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        let mut open = StandardResultSetOpenContext::new(
            context,
            Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())),
        );
        self.chain
            .standard_result_set_open_after_with_proxy(&mut open)
    }

    /// 委托实际异步关闭；参数为同一原生游标和上下文，返回未经改写的错误。
    pub(super) async fn result_set_close(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_close(raw, context).await
    }
}
