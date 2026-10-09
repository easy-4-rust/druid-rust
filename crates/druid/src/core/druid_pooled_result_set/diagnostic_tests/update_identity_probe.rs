use super::update_call::UpdateCall;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use std::sync::{Arc, Mutex};
/// 标准驱动探针保留 BASE 原始更新调用观察。对应 Java: java.sql.ResultSet。
#[derive(Debug)]
pub(super) struct UpdateIdentityProbe {
    calls: Arc<Mutex<Vec<UpdateCall>>>,
    closed: bool,
}
impl UpdateIdentityProbe {
    /// 保存原驱动调用记录；参数为共享记录，返回未关闭的驱动游标。
    pub(super) fn new(calls: Arc<Mutex<Vec<UpdateCall>>>) -> Self {
        Self {
            calls,
            closed: false,
        }
    }
}
druid_rdbc::impl_standard_wrapper!(UpdateIdentityProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for UpdateIdentityProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        if label == "mapped" {
            return ColumnIndex::new(2);
        }
        Err(RdbcError::InvalidArgument(format!(
            "label identity must reach driver: {label}"
        )))
    }
    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(UpdateCall::Index(column.get(), update));
        Ok(())
    }
    fn update_value_by_label(
        &mut self,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(UpdateCall::Label(label.to_owned(), update));
        Ok(())
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
