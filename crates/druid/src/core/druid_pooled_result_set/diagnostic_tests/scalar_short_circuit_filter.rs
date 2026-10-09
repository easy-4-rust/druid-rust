//! 完整标量读取原合同候选；未接入产品或Cargo。
use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead};
use std::sync::{Arc, Mutex};

/// 保存九类原重载调用。对应 Java: com.alibaba.druid.filter.Filter。
pub(super) struct ScalarShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}

impl ResultSetFilter for ScalarShortCircuitFilter {
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        let stem = match request {
            ResultSetRead::String => "string",
            ResultSetRead::Boolean => "boolean",
            ResultSetRead::Byte => "byte",
            ResultSetRead::Short => "short",
            ResultSetRead::Int => "int",
            ResultSetRead::Long => "long",
            ResultSetRead::Float => "float",
            ResultSetRead::Double => "double",
            ResultSetRead::Bytes => "bytes",
            _ => return next(),
        };
        let suffix = if label.is_some() { "_by_label" } else { "" };
        let target = label
            .map(str::to_owned)
            .unwrap_or_else(|| column.unwrap().get().to_string());
        self.calls
            .lock()
            .unwrap()
            .push(format!("result_set_get_{stem}{suffix}:{target}"));
        if matches!(request, ResultSetRead::Double) && label == Some("fail") {
            return Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "filtered double failure".to_string(),
            )));
        }
        let labeled = label.is_some();
        Ok(match request {
            ResultSetRead::String => Value::String(
                if labeled {
                    "filtered-label"
                } else {
                    "filtered-index"
                }
                .to_string(),
            ),
            ResultSetRead::Boolean => Value::Bool(!labeled),
            ResultSetRead::Byte => Value::Int(if labeled { 12 } else { 11 }),
            ResultSetRead::Short => Value::Int(if labeled { 22 } else { 21 }),
            ResultSetRead::Int => Value::Int(if labeled { 32 } else { 31 }),
            ResultSetRead::Long => Value::Int(if labeled { 42 } else { 41 }),
            ResultSetRead::Float => Value::Float(if labeled { 52.5 } else { 51.5 }),
            ResultSetRead::Double => Value::Float(if labeled { 62.5 } else { 61.5 }),
            ResultSetRead::Bytes => Value::Bytes(if labeled { vec![9, 10] } else { vec![7, 8] }),
            _ => unreachable!(),
        })
    }
}
