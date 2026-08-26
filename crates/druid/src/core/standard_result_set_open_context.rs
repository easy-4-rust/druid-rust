//! Canonical standard ResultSet 建立后的 Druid 私有可变 Filter 上下文。

use super::ResultSetFilterContext;
use druid_rdbc::metadata::ResultSetMetaData;
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) struct StandardResultSetOpenContext<'a> {
    filter_context: &'a ResultSetFilterContext,
    meta_data: Arc<dyn ResultSetMetaData>,
    logic_column_map: Option<HashMap<i32, i32>>,
    physical_column_map: Option<HashMap<i32, i32>>,
    hidden_columns: Option<Vec<i32>>,
}

impl<'a> StandardResultSetOpenContext<'a> {
    pub(crate) fn new(
        filter_context: &'a ResultSetFilterContext,
        meta_data: Arc<dyn ResultSetMetaData>,
    ) -> Self {
        Self {
            filter_context,
            meta_data,
            logic_column_map: None,
            physical_column_map: None,
            hidden_columns: None,
        }
    }

    pub(crate) const fn filter_context(&self) -> &ResultSetFilterContext {
        self.filter_context
    }

    pub(crate) fn meta_data(&self) -> &Arc<dyn ResultSetMetaData> {
        &self.meta_data
    }

    pub(crate) fn set_logic_column_map(&mut self, value: HashMap<i32, i32>) {
        self.logic_column_map = Some(value);
    }

    pub(crate) fn set_physical_column_map(&mut self, value: HashMap<i32, i32>) {
        self.physical_column_map = Some(value);
    }

    pub(crate) fn set_hidden_columns(&mut self, value: Vec<i32>) {
        self.hidden_columns = Some(value);
    }

    pub(crate) fn set_tenant_columns(&self, value: Vec<usize>) {
        self.filter_context.set_tenant_columns(value);
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
