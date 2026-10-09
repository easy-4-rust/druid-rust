use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead, ResultSetStreamKind};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::{Arc, Mutex};
/// 观察默认资源读取的全部原生重载，原 SQL NULL 与日志完整保留。
/// 对应 Java: java.sql.ResultSet 的十三类资源 getter。
pub(super) struct ResourceDefaultsResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: bool,
}
impl ResourceDefaultsResultSetProbe {
    /// 创建原生观察器；参数为共享日志，返回开放的结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            calls,
            closed: false,
        }
    }
    fn record(&self, family: &str, column: Option<ColumnIndex>, label: Option<&str>) {
        let event = match label {
            Some(label) => format!("physical:{family}_by_label:{label}"),
            None => format!(
                "physical:{family}:{}",
                column.expect("原下标必须保留").get()
            ),
        };
        self.calls.lock().unwrap().push(event);
    }
    fn resource_family(request: &ResultSetRead) -> Result<&'static str, RdbcError> {
        match request {
            ResultSetRead::Ref => Ok("reference"),
            ResultSetRead::Blob => Ok("blob"),
            ResultSetRead::Clob => Ok("clob"),
            ResultSetRead::Array => Ok("array"),
            ResultSetRead::Url => Ok("url"),
            ResultSetRead::RowId => Ok("row_id"),
            ResultSetRead::NClob => Ok("n_clob"),
            ResultSetRead::SqlXml => Ok("sql_xml"),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "resource_defaults_read",
            }),
        }
    }
    fn stream_family(kind: ResultSetStreamKind) -> Result<&'static str, RdbcError> {
        match kind {
            ResultSetStreamKind::Ascii => Ok("ascii_stream"),
            ResultSetStreamKind::Unicode => Ok("unicode_stream"),
            ResultSetStreamKind::Binary => Ok("binary_stream"),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "resource_defaults_stream",
            }),
        }
    }
}
druid_rdbc::impl_standard_wrapper!(ResourceDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ResourceDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "resource_defaults_next",
        })
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("physical:find_column:{label}"));
        ColumnIndex::new(3)
    }
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        self.record(Self::resource_family(request)?, Some(column), None);
        Ok(RdbcObject::Scalar(Value::Null))
    }
    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        self.record(Self::resource_family(request)?, None, Some(label));
        Ok(RdbcObject::Scalar(Value::Null))
    }
    fn input_stream(
        &self,
        column: ColumnIndex,
        kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.record(Self::stream_family(kind)?, Some(column), None);
        Ok(None)
    }
    fn input_stream_by_label(
        &self,
        label: &str,
        kind: ResultSetStreamKind,
    ) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.record(Self::stream_family(kind)?, None, Some(label));
        Ok(None)
    }
    fn reader(&self, column: ColumnIndex, national: bool) -> Result<Option<RdbcReader>, RdbcError> {
        self.record(
            if national {
                "n_character_stream"
            } else {
                "character_stream"
            },
            Some(column),
            None,
        );
        Ok(None)
    }
    fn reader_by_label(
        &self,
        label: &str,
        national: bool,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        self.record(
            if national {
                "n_character_stream"
            } else {
                "character_stream"
            },
            None,
            Some(label),
        );
        Ok(None)
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
