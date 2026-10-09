use crate::core::standard_result_set_open_context::StandardResultSetOpenContext;
use crate::core::{DruidError, ResultSetFilter};
use std::collections::HashMap;

/// 通过实际打开回调安装列映射。对应 Java: ResultSetProxyImpl 的 Filter 配置。
pub(super) struct InitialColumnMapFilter;

impl ResultSetFilter for InitialColumnMapFilter {
    fn standard_result_set_open_after_with_proxy(
        &self,
        context: &mut StandardResultSetOpenContext<'_>,
    ) -> Result<(), DruidError> {
        context.set_logic_column_map(HashMap::from([(1, 2), (2, 1)]));
        context.set_physical_column_map(HashMap::from([(2, 1), (1, 2)]));
        Ok(())
    }
}
