use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use std::sync::{Arc, Mutex};

/// 记录默认过滤器链实际送达的 LOB 流更新，保留原生重载身份且不消费资源。
/// 对应 Java: java.sql.ResultSet#updateBlob/updateClob/updateNClob。
#[derive(Debug)]
pub(super) struct LobStreamDefaultsResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: bool,
}

impl LobStreamDefaultsResultSetProbe {
    /// 创建原生更新探针；参数为共享日志，返回未关闭的标准结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            calls,
            closed: false,
        }
    }

    fn record(
        &self,
        column: String,
        by_label: bool,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        // 只观察完整描述符，不读取、关闭或替换调用者的共享 stream/reader。
        let (method, present, length) = match update {
            ResultSetUpdate::BlobStream { stream, length } => (
                "update_blob_stream",
                stream.is_some(),
                format!("{length:?}"),
            ),
            ResultSetUpdate::ClobReader { reader, length } => (
                "update_clob_reader",
                reader.is_some(),
                format!("{length:?}"),
            ),
            ResultSetUpdate::NClobReader { reader, length } => (
                "update_n_clob_reader",
                reader.is_some(),
                format!("{length:?}"),
            ),
            _ => {
                return Err(RdbcError::InvalidArgument(
                    "LOB default probe requires a LOB stream descriptor".to_owned(),
                ))
            }
        };
        let suffix = if by_label { "_by_label" } else { "" };
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "physical:{method}{suffix}:{column}:{present}:{length}"
            ));
        Ok(())
    }
}

druid_rdbc::impl_standard_wrapper!(LobStreamDefaultsResultSetProbe => ResultSet);

#[async_trait::async_trait]
impl ResultSet for LobStreamDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("physical:next".to_owned());
        Ok(false)
    }

    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.record(column.get().to_string(), false, update)
    }

    fn update_value_by_label(
        &mut self,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.record(label.to_owned(), true, update)
    }

    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("physical:find_column:{label}"));
        ColumnIndex::new(23)
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("physical:close".to_owned());
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
