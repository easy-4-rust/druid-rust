use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use std::sync::{Arc, Mutex};
/// 观察原生资源更新十四重载的完整参数，不改写 NULL 或 RowId 身份。
/// 对应 Java: java.sql.ResultSet#updateRef/updateBlob/updateClob/updateArray/updateRowId/updateNClob/updateSQLXML。
pub(super) struct ResourceUpdateDefaultsResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: bool,
}
impl ResourceUpdateDefaultsResultSetProbe {
    /// 创建开放探针；参数为共享日志，返回结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            calls,
            closed: false,
        }
    }
    fn record(
        &self,
        column: &str,
        by_label: bool,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        let (method, value) = match update {
            ResultSetUpdate::Ref(value) => ("update_reference", format!("{value:?}")),
            ResultSetUpdate::Blob(value) => ("update_blob", format!("{value:?}")),
            ResultSetUpdate::Clob(value) => ("update_clob", format!("{value:?}")),
            ResultSetUpdate::Array(value) => ("update_array", format!("{value:?}")),
            ResultSetUpdate::RowId(value) => ("update_row_id", format!("{value:?}")),
            ResultSetUpdate::NClob(value) => ("update_n_clob", format!("{value:?}")),
            ResultSetUpdate::SqlXml(value) => ("update_sql_xml", format!("{value:?}")),
            _ => {
                return Err(RdbcError::InvalidArgument(
                    "需要原资源更新描述符".to_owned(),
                ))
            }
        };
        let suffix = if by_label { "_by_label" } else { "" };
        self.calls
            .lock()
            .unwrap()
            .push(format!("physical:{method}{suffix}:{column}:{value}"));
        Ok(())
    }
}
druid_rdbc::impl_standard_wrapper!(ResourceUpdateDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ResourceUpdateDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "resource_update_defaults_next",
        })
    }
    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.record(&column.get().to_string(), false, update)
    }
    fn update_value_by_label(
        &mut self,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.record(label, true, update)
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("physical:find_column:{label}"));
        ColumnIndex::new(23)
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
