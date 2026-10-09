use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use std::sync::{Arc, Mutex};
/// 观察流更新原始重载和长度，不读取、关闭或改写调用者资源。
/// 对应 Java: java.sql.ResultSet#updateAsciiStream/updateBinaryStream/updateCharacterStream/updateNCharacterStream。
pub(super) struct StreamUpdateDefaultsResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: bool,
}
impl StreamUpdateDefaultsResultSetProbe {
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
        // 保留原描述符身份，只观察可空句柄与长度，不提前消费共享流。
        let (family, present, length) = match update {
            ResultSetUpdate::AsciiStream { stream, length } => {
                ("AsciiStream", stream.is_some(), format!("{length:?}"))
            }
            ResultSetUpdate::BinaryStream { stream, length } => {
                ("BinaryStream", stream.is_some(), format!("{length:?}"))
            }
            ResultSetUpdate::CharacterStream { reader, length } => {
                ("CharacterStream", reader.is_some(), format!("{length:?}"))
            }
            ResultSetUpdate::NCharacterStream { reader, length } => {
                ("NCharacterStream", reader.is_some(), format!("{length:?}"))
            }
            _ => return Err(RdbcError::InvalidArgument("需要原流更新描述符".to_owned())),
        };
        let method = if by_label {
            "update_value_by_label"
        } else {
            "update_value"
        };
        self.calls.lock().unwrap().push(format!(
            "physical:{method}:{column}:{family}:{present}:{length}"
        ));
        Ok(())
    }
}
druid_rdbc::impl_standard_wrapper!(StreamUpdateDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for StreamUpdateDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "stream_update_defaults_next",
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
