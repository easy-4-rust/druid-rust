//! 保留原 NString 索引与标签驱动重载的独立观察。
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::{Arc, Mutex};

/// 区分通用对象、NString 索引及 NString 标签路径。对应 Java: java.sql.ResultSet。
#[derive(Debug)]
pub(super) struct NStringReadProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: bool,
}

impl NStringReadProbe {
    /// 创建驱动探针；参数为共享记录，返回未关闭游标。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            calls,
            closed: false,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(NStringReadProbe => ResultSet);

#[async_trait::async_trait]
impl ResultSet for NStringReadProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "n_string_probe_next",
        })
    }

    fn object(&self, _column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        Ok(RdbcObject::Scalar(Value::String("通用对象入口".to_owned())))
    }

    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("physical:find_column:{label}"));
        ColumnIndex::new(23)
    }

    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        assert_eq!(request, &ResultSetRead::NString);
        self.calls
            .lock()
            .unwrap()
            .push(format!("physical:n_string:{}", column.get()));
        Ok(RdbcObject::NString("索引-NString".to_owned()))
    }

    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        assert_eq!(request, &ResultSetRead::NString);
        self.calls
            .lock()
            .unwrap()
            .push(format!("physical:n_string_by_label:{label}"));
        Ok(RdbcObject::NString("标签-NString".to_owned()))
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
