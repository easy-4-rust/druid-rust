use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use std::sync::{Arc, Mutex};

/// 观察 NString 更新的进入、反向退出以及短路，不接管原生游标。
/// 对应 Java: Filter#resultSet_updateNString。
pub(super) struct OrderedNStringUpdateFilter {
    pub(super) name: &'static str,
    pub(super) mode: u8,
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}

impl OrderedNStringUpdateFilter {
    fn invoke(
        &self,
        target: &str,
        update: ResultSetUpdate,
        next: &mut dyn FnMut(ResultSetUpdate) -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        if !matches!(update, ResultSetUpdate::NString(_)) {
            return next(update);
        }
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:{target}:before", self.name));
        let result = match self.mode {
            1 => Ok(()),
            2 => Err(DruidError::DriverError(
                "ordered NString update failure".to_owned(),
            )),
            _ => next(update),
        };
        let status = if result.is_ok() { "ok" } else { "error" };
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:{target}:{status}", self.name));
        result
    }
}

#[async_trait::async_trait]
impl ResultSetFilter for OrderedNStringUpdateFilter {
    fn standard_result_set_update(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        self.invoke(&format!("index:{}", column.get()), update, &mut |update| {
            chain.result_set_update(column, update)
        })
    }
    fn standard_result_set_update_by_label(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        self.invoke(&format!("label:{label}"), update, &mut |update| {
            chain.result_set_update_by_label(label, update)
        })
    }
}
