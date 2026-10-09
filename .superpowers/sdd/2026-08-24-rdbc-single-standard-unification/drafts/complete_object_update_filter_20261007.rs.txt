use super::object_update_vendor_probe::FilterVendorObjectProbe;
use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use druid_rdbc::types::{RdbcObject, RdbcOpaqueObject};
use std::sync::{Arc, Mutex};

/// 保留普通与 scaleOrLength 的四种独立对象更新观察。
/// 对应 Java: Filter#resultSet_updateObject 的四种重载。
pub(super) struct ObjectUpdateShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) expected_custom: RdbcOpaqueObject,
}

impl ObjectUpdateShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

#[async_trait::async_trait]
impl ResultSetFilter for ObjectUpdateShortCircuitFilter {
    fn standard_result_set_update(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        match update {
            ResultSetUpdate::Object(value) => {
                assert_eq!(value, RdbcObject::Custom(self.expected_custom.clone()));
                let RdbcObject::Custom(custom) = &value else {
                    panic!("updateObject 必须保留 vendor custom 对象分支");
                };
                let vendor = custom
                    .downcast_ref::<FilterVendorObjectProbe>()
                    .expect("必须保留具体 vendor 对象");
                assert_eq!(vendor.id, 99);
                self.record(format!(
                    "result_set_update_object:{}:custom:{}:{}",
                    column.get(),
                    custom.class_name(),
                    vendor.id
                ));
                Ok(())
            }
            ResultSetUpdate::ObjectWithScaleOrLength {
                value,
                scale_or_length,
            } => {
                self.record(format!(
                    "result_set_update_object_with_scale_or_length:{}:{value:?}:{scale_or_length}",
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
        column_label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), DruidError> {
        match update {
            ResultSetUpdate::Object(value) => {
                self.record(format!(
                    "result_set_update_object_by_label:{column_label}:{value:?}"
                ));
                if column_label == "fail" {
                    Err(DruidError::DriverError(
                        "filtered updateObject failure".to_owned(),
                    ))
                } else {
                    Ok(())
                }
            }
            ResultSetUpdate::ObjectWithScaleOrLength {
                value,
                scale_or_length,
            } => {
                self.record(format!("result_set_update_object_by_label_with_scale_or_length:{column_label}:{value:?}:{scale_or_length}"));
                Ok(())
            }
            other => chain.result_set_update_by_label(column_label, other),
        }
    }
}
