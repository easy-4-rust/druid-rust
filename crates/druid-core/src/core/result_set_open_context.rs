//! `ResultSet` 建立后的可变 Filter 上下文。
//!
//! 这是 Rust 对 Java `ResultSetProxy` 可变列映射能力的协议化承载。Java Filter
//! 直接修改代理；Rust 在代理完成构造前收集映射，避免自引用和可变别名。

use super::{PhysicalResultSet, ResultSetFilterContext};
use std::collections::HashMap;

/// `ResultSet` open-after Filter 可读取的物理资源及可写列映射。
pub struct ResultSetOpenContext<'a> {
    filter_context: &'a ResultSetFilterContext,
    physical: &'a dyn PhysicalResultSet,
    logic_column_map: Option<HashMap<i32, i32>>,
    physical_column_map: Option<HashMap<i32, i32>>,
    hidden_columns: Option<Vec<i32>>,
}

impl<'a> ResultSetOpenContext<'a> {
    /// 为刚创建的 `ResultSet` 建立 open-after 上下文。
    pub(crate) fn new(
        filter_context: &'a ResultSetFilterContext,
        physical: &'a dyn PhysicalResultSet,
    ) -> Self {
        Self {
            filter_context,
            physical,
            logic_column_map: None,
            physical_column_map: None,
            hidden_columns: None,
        }
    }

    /// 返回只读的 `ResultSet` 身份和统计上下文。
    #[must_use]
    pub const fn filter_context(&self) -> &ResultSetFilterContext {
        self.filter_context
    }

    /// 返回底层物理 `ResultSet`。
    ///
    /// 对应 Java：`ResultSetProxy#getResultSetRaw()`。
    #[must_use]
    pub const fn raw_result_set(&self) -> &dyn PhysicalResultSet {
        self.physical
    }

    /// 设置逻辑列到物理列映射。
    pub fn set_logic_column_map(&mut self, logic_column_map: HashMap<i32, i32>) {
        self.logic_column_map = Some(logic_column_map);
    }

    /// 设置物理列到逻辑列映射。
    pub fn set_physical_column_map(&mut self, physical_column_map: HashMap<i32, i32>) {
        self.physical_column_map = Some(physical_column_map);
    }

    /// 设置需要从逻辑 `ResultSet` 隐藏的 1-based 物理列。
    pub fn set_hidden_columns(&mut self, hidden_columns: Vec<i32>) {
        self.hidden_columns = Some(hidden_columns);
    }

    /// 保存成功移动行后需要回调的租户列。
    pub fn set_tenant_columns(&self, tenant_columns: Vec<usize>) {
        self.filter_context.set_tenant_columns(tenant_columns);
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn into_column_mappings(
        self,
    ) -> (
        Option<HashMap<i32, i32>>,
        Option<HashMap<i32, i32>>,
        Option<Vec<i32>>,
    ) {
        (
            self.logic_column_map,
            self.physical_column_map,
            self.hidden_columns,
        )
    }
}
