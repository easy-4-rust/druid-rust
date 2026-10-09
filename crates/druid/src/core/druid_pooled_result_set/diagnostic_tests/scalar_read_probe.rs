//! 九类标量原生下标/标签重载的独立观察候选，未接入Cargo。
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::{Arc, Mutex};

/// 原生读取独立探针，通用object不能伪装强类型委托。对应 Java: java.sql.ResultSet。
#[derive(Debug)]
pub(super) struct ScalarReadProbe {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) closed: bool,
}

pub(super) fn scalar_stem(request: &ResultSetRead) -> &'static str {
    match request {
        ResultSetRead::String => "string",
        ResultSetRead::Boolean => "boolean",
        ResultSetRead::Byte => "byte",
        ResultSetRead::Short => "short",
        ResultSetRead::Int => "int",
        ResultSetRead::Long => "long",
        ResultSetRead::Float => "float",
        ResultSetRead::Double => "double",
        ResultSetRead::Bytes => "bytes",
        _ => panic!("probe accepts only the nine scalar BASE families"),
    }
}

impl ScalarReadProbe {
    fn read(
        &self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        let stem = scalar_stem(request);
        let suffix = if label.is_some() { "_by_label" } else { "" };
        let target = label
            .map(str::to_owned)
            .unwrap_or_else(|| column.unwrap().get().to_string());
        self.calls
            .lock()
            .unwrap()
            .push(format!("raw:{stem}{suffix}:{target}"));
        if matches!(request, ResultSetRead::Double) && label == Some("fail") {
            return Err(RdbcError::DriverError("scalar probe failure".to_owned()));
        }
        Ok(RdbcObject::Scalar(match request {
            ResultSetRead::String => Value::String("原生😀字符串".to_owned()),
            ResultSetRead::Boolean => Value::Bool(true),
            ResultSetRead::Byte => Value::Int(11),
            ResultSetRead::Short => Value::Int(21),
            ResultSetRead::Int => Value::Int(31),
            ResultSetRead::Long => Value::Int(41),
            ResultSetRead::Float => Value::Float(51.5),
            ResultSetRead::Double => Value::Float(61.5),
            ResultSetRead::Bytes => Value::Bytes(vec![7, 8]),
            _ => unreachable!(),
        }))
    }
}

druid_rdbc::impl_standard_wrapper!(ScalarReadProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ScalarReadProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "scalar_probe_next",
        })
    }
    fn object(&self, _column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push("unexpected:object".to_owned());
        Err(RdbcError::DriverError(
            "unexpected generic object read".to_owned(),
        ))
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls.lock().unwrap().push(format!("raw:find:{label}"));
        ColumnIndex::new(23)
    }
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        self.read(Some(column), None, request)
    }
    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        self.read(None, Some(label), request)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
