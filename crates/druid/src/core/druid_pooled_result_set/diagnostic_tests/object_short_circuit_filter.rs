//! 完整对象重载的短路与参数身份测试夹具。
use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::types::{RdbcObject, RdbcTypeMap};
use std::sync::{Arc, Mutex};

use crate::core::ResultSetObjectRead;

/// 保存完整对象读取事件及原始参数地址。
/// 对应 Java: com.alibaba.druid.filter.Filter 的 ResultSet#getObject 重载。
pub(super) struct ObjectShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) arguments: Arc<Mutex<Vec<usize>>>,
}

impl ResultSetFilter for ObjectShortCircuitFilter {
    fn standard_result_set_object(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetObjectRead<'_>,
        _next: &mut dyn FnMut() -> Result<RdbcObject, DruidError>,
    ) -> Result<RdbcObject, DruidError> {
        let target = label
            .map(str::to_owned)
            .unwrap_or_else(|| column.unwrap().get().to_string());
        let (call, object) = match request {
            ResultSetObjectRead::Ordinary => match label {
                Some(_) => (
                    format!("result_set_get_object_by_label:{target}"),
                    RdbcObject::Scalar(Value::String("filtered-label-object".to_owned())),
                ),
                None => (
                    format!("result_set_get_object:{target}"),
                    RdbcObject::Scalar(Value::Int(901)),
                ),
            },
            ResultSetObjectRead::TypeMap(map) => {
                if let Some(map) = map {
                    self.arguments
                        .lock()
                        .unwrap()
                        .push(std::ptr::from_ref(*map) as usize);
                }
                let identity = Self::type_map_identity(*map);
                if label.is_some() {
                    (
                        format!("result_set_get_object_by_label_with_type_map:{target}:{identity}"),
                        RdbcObject::String(format!("filtered-label-map:{identity}")),
                    )
                } else {
                    (
                        format!("result_set_get_object_with_type_map:{target}:{identity}"),
                        RdbcObject::String(format!("filtered-map:{identity}")),
                    )
                }
            }
            ResultSetObjectRead::Typed(target_type) => {
                self.arguments
                    .lock()
                    .unwrap()
                    .push(std::ptr::from_ref(*target_type) as usize);
                if label.is_some() {
                    (
                        format!("result_set_get_object_typed_by_label:{target}:{target_type:?}"),
                        RdbcObject::String(format!("filtered-label-typed:{target_type:?}")),
                    )
                } else {
                    (
                        format!("result_set_get_object_typed:{target}:{target_type:?}"),
                        RdbcObject::String(format!("filtered-typed:{target_type:?}")),
                    )
                }
            }
        };
        self.calls.lock().unwrap().push(call);
        if label == Some("fail") {
            Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "filtered object failure".to_owned(),
            )))
        } else {
            Ok(object)
        }
    }
}

impl ObjectShortCircuitFilter {
    fn type_map_identity(type_map: Option<&RdbcTypeMap>) -> String {
        match type_map {
            None => "null".to_owned(),
            Some(type_map) if type_map.is_empty() => "empty".to_owned(),
            Some(type_map) => {
                let mut entries = type_map
                    .mappings()
                    .iter()
                    .map(|(name, target)| format!("{name}={target:?}"))
                    .collect::<Vec<_>>();
                entries.sort();
                entries.join(",")
            }
        }
    }
}
