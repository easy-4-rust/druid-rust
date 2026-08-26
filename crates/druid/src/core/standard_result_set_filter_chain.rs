//! Canonical async ResultSet 的 Druid around-chain。

use super::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use druid_rdbc::result_set::ColumnIndex;
use std::sync::Arc;

/// 让现有 ResultSetFilter 以相同注册顺序包围 async standard cursor。
pub(crate) struct StandardResultSetFilterChain<'a> {
    filters: &'a [Arc<dyn ResultSetFilter>],
    position: usize,
    standard: &'a mut dyn druid_rdbc::result_set::ResultSet,
    context: &'a ResultSetFilterContext,
}

impl<'a> StandardResultSetFilterChain<'a> {
    pub(crate) fn new(
        filters: &'a [Arc<dyn ResultSetFilter>],
        standard: &'a mut dyn druid_rdbc::result_set::ResultSet,
        context: &'a ResultSetFilterContext,
    ) -> Self {
        Self {
            filters,
            position: 0,
            standard,
            context,
        }
    }

    pub(crate) async fn result_set_next(&mut self) -> Result<bool, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_next(self).await
        } else {
            self.standard.next().await.map_err(DruidError::from)
        }
    }

    pub(crate) async fn result_set_close(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.standard_result_set_close(self).await
        } else {
            self.standard.close().await.map_err(DruidError::from)
        }
    }

    pub(crate) fn context(&self) -> &ResultSetFilterContext {
        self.context
    }

    pub(crate) fn raw_value(&self, column_index: usize) -> Result<Value, DruidError> {
        let column = ColumnIndex::new(column_index).map_err(DruidError::from)?;
        self.standard.value(column).map_err(DruidError::from)
    }
}
