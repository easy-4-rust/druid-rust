use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext, ResultSetRowOperation};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 按原独立行操作记录短路，并可切换refreshRow精确错误。
/// 对应 Java: Filter#resultSet_insertRow/updateRow/deleteRow/refreshRow/cancelRowUpdates/moveToInsertRow/moveToCurrentRow。
pub(super) struct RowMutationShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) fail_refresh: Arc<AtomicBool>,
}

#[async_trait::async_trait]
impl ResultSetFilter for RowMutationShortCircuitFilter {
    fn standard_result_set_row_operation(
        &self,
        _context: &ResultSetFilterContext,
        operation: ResultSetRowOperation,
        _next: &mut dyn FnMut() -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        let name = match operation {
            ResultSetRowOperation::InsertRow => "insert_row",
            ResultSetRowOperation::UpdateRow => "update_row",
            ResultSetRowOperation::DeleteRow => "delete_row",
            ResultSetRowOperation::RefreshRow => "refresh_row",
            ResultSetRowOperation::CancelRowUpdates => "cancel_row_updates",
            ResultSetRowOperation::MoveToInsertRow => "move_to_insert_row",
            ResultSetRowOperation::MoveToCurrentRow => "move_to_current_row",
        };
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(name.to_owned());
        if operation == ResultSetRowOperation::RefreshRow
            && self.fail_refresh.load(Ordering::Acquire)
        {
            Err(DruidError::DriverError(
                "filtered refreshRow failure".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}
