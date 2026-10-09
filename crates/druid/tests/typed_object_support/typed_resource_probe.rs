use super::typed_resource_call::TypedResourceCall;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::Mutex;

/// 记录唯一标准 ResultSet 的资源读取分派，不提供通用 value 替代入口。
/// 对应 Java: java.sql.ResultSet#getObject(int, Class)。
#[derive(Debug)]
pub struct TypedResourceProbe {
    calls: Mutex<Vec<TypedResourceCall>>,
}

impl TypedResourceProbe {
    /// 创建空调用轨迹；返回探针。
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    /// 返回实际调用轨迹快照；无参数。
    pub fn calls(&self) -> Vec<TypedResourceCall> {
        self.calls.lock().unwrap().clone()
    }
}

druid_rdbc::impl_standard_wrapper!(TypedResourceProbe => ResultSet);

#[async_trait::async_trait]
impl ResultSet for TypedResourceProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::FindColumn(label.to_string()));
        ColumnIndex::new(41)
    }

    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        let index = column.get();
        let call = match request {
            ResultSetRead::Blob => TypedResourceCall::Blob(index),
            ResultSetRead::Clob => TypedResourceCall::Clob(index),
            ResultSetRead::NClob => TypedResourceCall::NClob(index),
            ResultSetRead::Array => TypedResourceCall::Array(index),
            ResultSetRead::Ref => TypedResourceCall::Ref(index),
            ResultSetRead::RowId => TypedResourceCall::RowId(index),
            ResultSetRead::SqlXml => TypedResourceCall::SqlXml(index),
            ResultSetRead::Url => TypedResourceCall::Url(index),
            _ => {
                return Err(RdbcError::InvalidArgument(
                    "expected resource getter".to_owned(),
                ))
            }
        };
        self.calls.lock().unwrap().push(call);
        Ok(RdbcObject::Scalar(Value::Null))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}
