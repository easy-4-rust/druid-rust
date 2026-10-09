use crate::core::{
    DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext, ResultSetRowOperation,
};
use druid_rdbc::result_set::ResultSet;
use std::sync::Arc;
/// 原行修改入口通过实际标准过滤器链惰性委托到同名驱动方法。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct RowMutationDefaultsChainHarness {
    chain: FilterChain,
}
impl RowMutationDefaultsChainHarness {
    /// 创建链桥接；无参数，返回空链。
    pub(super) fn new() -> Self {
        Self {
            chain: FilterChain::new(),
        }
    }
    /// 注册原顺序过滤器；参数为过滤器，返回完成状态。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) {
        self.chain.add_result_set(filter);
    }
    /// 委托原行修改；参数为驱动和上下文，返回真实过滤器或驱动结果。
    pub(super) fn result_set_insert_row(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_row_operation(
            context,
            ResultSetRowOperation::InsertRow,
            &mut || raw.insert_row().map_err(DruidError::from),
        )
    }
    /// 委托原行修改；参数为驱动和上下文，返回真实过滤器或驱动结果。
    pub(super) fn result_set_update_row(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_row_operation(
            context,
            ResultSetRowOperation::UpdateRow,
            &mut || raw.update_row().map_err(DruidError::from),
        )
    }
    /// 委托原行修改；参数为驱动和上下文，返回真实过滤器或驱动结果。
    pub(super) fn result_set_delete_row(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_row_operation(
            context,
            ResultSetRowOperation::DeleteRow,
            &mut || raw.delete_row().map_err(DruidError::from),
        )
    }
    /// 委托原行修改；参数为驱动和上下文，返回真实过滤器或驱动结果。
    pub(super) fn result_set_refresh_row(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_row_operation(
            context,
            ResultSetRowOperation::RefreshRow,
            &mut || raw.refresh_row().map_err(DruidError::from),
        )
    }
    /// 委托原行修改；参数为驱动和上下文，返回真实过滤器或驱动结果。
    pub(super) fn result_set_cancel_row_updates(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_row_operation(
            context,
            ResultSetRowOperation::CancelRowUpdates,
            &mut || raw.cancel_row_updates().map_err(DruidError::from),
        )
    }
    /// 委托原行修改；参数为驱动和上下文，返回真实过滤器或驱动结果。
    pub(super) fn result_set_move_to_insert_row(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_row_operation(
            context,
            ResultSetRowOperation::MoveToInsertRow,
            &mut || raw.move_to_insert_row().map_err(DruidError::from),
        )
    }
    /// 委托原行修改；参数为驱动和上下文，返回真实过滤器或驱动结果。
    pub(super) fn result_set_move_to_current_row(
        &self,
        raw: &mut dyn ResultSet,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.chain.standard_result_set_row_operation(
            context,
            ResultSetRowOperation::MoveToCurrentRow,
            &mut || raw.move_to_current_row().map_err(DruidError::from),
        )
    }
}
