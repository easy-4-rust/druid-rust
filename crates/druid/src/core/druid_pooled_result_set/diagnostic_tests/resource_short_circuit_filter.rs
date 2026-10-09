use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead, ResultSetStreamKind};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::{Arc, Mutex};

/// 保留十三种资源 getter 的下标/标签身份和共享流游标。
/// 对应 Java: Filter#resultSet_getRef/getBlob/getClob/getArray/getURL/getRowId/getNClob/getSQLXML/getXxxStream。
pub(super) struct ResourceShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) stream: RdbcInputStream,
    pub(super) reader: RdbcReader,
}

impl ResourceShortCircuitFilter {
    fn record(&self, stem: &str, column: Option<ColumnIndex>, label: Option<&str>) {
        let call = match label {
            Some(label) => format!("{stem}_by_label:{label}"),
            None => format!("{stem}:{}", column.expect("下标重载必须保留下标").get()),
        };
        self.calls.lock().unwrap().push(call);
    }
}

#[async_trait::async_trait]
impl ResultSetFilter for ResourceShortCircuitFilter {
    fn standard_result_set_resource(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<RdbcObject, DruidError>,
    ) -> Result<RdbcObject, DruidError> {
        let stem = match request {
            ResultSetRead::Ref => "result_set_get_ref",
            ResultSetRead::Blob => "result_set_get_blob",
            ResultSetRead::Clob => "result_set_get_clob",
            ResultSetRead::Array => "result_set_get_array",
            ResultSetRead::Url => "result_set_get_url",
            ResultSetRead::RowId => "result_set_get_row_id",
            ResultSetRead::NClob => "result_set_get_n_clob",
            ResultSetRead::SqlXml => "result_set_get_sql_xml",
            _ => return next(),
        };
        self.record(stem, column, label);
        if matches!(request, ResultSetRead::SqlXml) && label == Some("fail") {
            return Err(DruidError::DriverError(
                "filtered SQLXML failure".to_owned(),
            ));
        }
        Ok(RdbcObject::Scalar(Value::Null))
    }

    fn standard_result_set_input_stream(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        kind: ResultSetStreamKind,
        next: &mut dyn FnMut() -> Result<Option<RdbcInputStream>, DruidError>,
    ) -> Result<Option<RdbcInputStream>, DruidError> {
        let stem = match kind {
            ResultSetStreamKind::Ascii => "result_set_get_ascii_stream",
            ResultSetStreamKind::Unicode => "result_set_get_unicode_stream",
            ResultSetStreamKind::Binary => "result_set_get_binary_stream",
            _ => return next(),
        };
        self.record(stem, column, label);
        Ok(matches!(kind, ResultSetStreamKind::Ascii).then(|| self.stream.clone()))
    }

    fn standard_result_set_reader(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        national: bool,
        _next: &mut dyn FnMut() -> Result<Option<RdbcReader>, DruidError>,
    ) -> Result<Option<RdbcReader>, DruidError> {
        let stem = if national {
            "result_set_get_n_character_stream"
        } else {
            "result_set_get_character_stream"
        };
        self.record(stem, column, label);
        Ok((!national).then(|| self.reader.clone()))
    }
}
