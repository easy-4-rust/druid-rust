use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use std::sync::{Arc, Mutex};

/// 保留七类资源下标和标签更新的原始事件及短路错误。
/// 对应 Java: Filter#resultSet_updateRef/updateBlob/updateClob/updateArray/updateRowId/updateNClob/updateSQLXML。
pub(super) struct ResourceUpdateShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}

impl ResourceUpdateShortCircuitFilter {
    fn record(&self, event: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }
}

#[async_trait::async_trait]
impl ResultSetFilter for ResourceUpdateShortCircuitFilter {
    fn standard_result_set_update(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        match update {
            ResultSetUpdate::Ref(value) => {
                self.record(format!(
                    "result_set_update_reference:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::Blob(value) => {
                self.record(format!("result_set_update_blob:{}:{value:?}", column.get()));
                Ok(())
            }
            ResultSetUpdate::Clob(value) => {
                self.record(format!("result_set_update_clob:{}:{value:?}", column.get()));
                Ok(())
            }
            ResultSetUpdate::Array(value) => {
                self.record(format!(
                    "result_set_update_array:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::RowId(value) => {
                self.record(format!(
                    "result_set_update_row_id:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::NClob(value) => {
                self.record(format!(
                    "result_set_update_n_clob:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::SqlXml(value) => {
                self.record(format!(
                    "result_set_update_sql_xml:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            other => chain.result_set_update(column, other),
        }
    }
    fn standard_result_set_update_by_label(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        match update {
            ResultSetUpdate::Ref(value) => {
                self.record(format!(
                    "result_set_update_reference_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered resource update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Blob(value) => {
                self.record(format!("result_set_update_blob_by_label:{label}:{value:?}"));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered resource update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Clob(value) => {
                self.record(format!("result_set_update_clob_by_label:{label}:{value:?}"));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered resource update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Array(value) => {
                self.record(format!(
                    "result_set_update_array_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered resource update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::RowId(value) => {
                self.record(format!(
                    "result_set_update_row_id_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered resource update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::NClob(value) => {
                self.record(format!(
                    "result_set_update_n_clob_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered resource update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::SqlXml(value) => {
                self.record(format!(
                    "result_set_update_sql_xml_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered resource update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            other => chain.result_set_update_by_label(label, other),
        }
    }
}
