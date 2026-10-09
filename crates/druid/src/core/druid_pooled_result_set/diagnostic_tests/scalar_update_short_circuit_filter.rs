use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use std::sync::{Arc, Mutex};

/// 完整保留十四类标量下标和标签更新的原事件、参数与短路错误。
/// 对应 Java: Filter#resultSet_updateNull/updateBoolean/updateByte/updateShort/updateInt/updateLong/updateFloat/updateDouble/updateBigDecimal/updateString/updateBytes/updateDate/updateTime/updateTimestamp。
pub(super) struct ScalarUpdateShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}
impl ScalarUpdateShortCircuitFilter {
    fn record(&self, event: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }
}
#[async_trait::async_trait]
impl ResultSetFilter for ScalarUpdateShortCircuitFilter {
    fn standard_result_set_update(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        match update {
            ResultSetUpdate::Null => {
                self.record(format!("result_set_update_null:{}", column.get()));
                Ok(())
            }
            ResultSetUpdate::Boolean(value) => {
                self.record(format!(
                    "result_set_update_boolean:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::Byte(value) => {
                self.record(format!("result_set_update_byte:{}:{value:?}", column.get()));
                Ok(())
            }
            ResultSetUpdate::Short(value) => {
                self.record(format!(
                    "result_set_update_short:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::Int(value) => {
                self.record(format!("result_set_update_int:{}:{value:?}", column.get()));
                Ok(())
            }
            ResultSetUpdate::Long(value) => {
                self.record(format!("result_set_update_long:{}:{value:?}", column.get()));
                Ok(())
            }
            ResultSetUpdate::Float(value) => {
                self.record(format!(
                    "result_set_update_float:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::Double(value) => {
                self.record(format!(
                    "result_set_update_double:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::BigDecimal(value) => {
                self.record(format!(
                    "result_set_update_big_decimal:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::String(value) => {
                self.record(format!(
                    "result_set_update_string:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::Bytes(value) => {
                self.record(format!(
                    "result_set_update_bytes:{}:{value:?}",
                    column.get()
                ));
                Ok(())
            }
            ResultSetUpdate::Date(value) => {
                self.record(format!("result_set_update_date:{}:{value:?}", column.get()));
                Ok(())
            }
            ResultSetUpdate::Time(value) => {
                self.record(format!("result_set_update_time:{}:{value:?}", column.get()));
                Ok(())
            }
            ResultSetUpdate::Timestamp(value) => {
                self.record(format!(
                    "result_set_update_timestamp:{}:{value:?}",
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
            ResultSetUpdate::Null => {
                self.record(format!("result_set_update_null_by_label:{label}"));
                Ok(())
            }
            ResultSetUpdate::Boolean(value) => {
                self.record(format!(
                    "result_set_update_boolean_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Byte(value) => {
                self.record(format!("result_set_update_byte_by_label:{label}:{value:?}"));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Short(value) => {
                self.record(format!(
                    "result_set_update_short_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Int(value) => {
                self.record(format!("result_set_update_int_by_label:{label}:{value:?}"));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Long(value) => {
                self.record(format!("result_set_update_long_by_label:{label}:{value:?}"));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Float(value) => {
                self.record(format!(
                    "result_set_update_float_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Double(value) => {
                self.record(format!(
                    "result_set_update_double_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::BigDecimal(value) => {
                self.record(format!(
                    "result_set_update_big_decimal_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::String(value) => {
                self.record(format!(
                    "result_set_update_string_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Bytes(value) => {
                self.record(format!(
                    "result_set_update_bytes_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Date(value) => {
                self.record(format!("result_set_update_date_by_label:{label}:{value:?}"));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Time(value) => {
                self.record(format!("result_set_update_time_by_label:{label}:{value:?}"));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::Timestamp(value) => {
                self.record(format!(
                    "result_set_update_timestamp_by_label:{label}:{value:?}"
                ));
                if label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered scalar update failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            other => chain.result_set_update_by_label(label, other),
        }
    }
}
