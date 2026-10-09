use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use std::sync::{Arc, Mutex};

/// 保留原 NString 更新两种重载、可空值及精确短路错误。
/// 对应 Java: Filter#resultSet_updateNString。
pub(super) struct NStringUpdateShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl ResultSetFilter for NStringUpdateShortCircuitFilter {
    fn standard_result_set_update(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        if let ResultSetUpdate::NString(value) = update {
            self.calls.lock().unwrap().push(format!(
                "result_set_update_n_string:{}:{value:?}",
                column.get()
            ));
            Ok(())
        } else {
            chain.result_set_update(column, update)
        }
    }

    fn standard_result_set_update_by_label(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        if let ResultSetUpdate::NString(value) = update {
            self.calls.lock().unwrap().push(format!(
                "result_set_update_n_string_by_label:{label}:{value:?}"
            ));
            if label == "fail" {
                Err(DruidError::DriverError(
                    "filtered NString update failure".to_owned(),
                ))
            } else {
                Ok(())
            }
        } else {
            chain.result_set_update_by_label(label, update)
        }
    }
}
