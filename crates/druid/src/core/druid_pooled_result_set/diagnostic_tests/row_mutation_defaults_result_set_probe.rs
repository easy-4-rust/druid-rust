use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::ResultSet;
use std::sync::{Arc, Mutex};
/// 观察七个原生无参行修改方法，完整保留原调用顺序。
/// 对应 Java: java.sql.ResultSet 的七个行修改方法。
pub(super) struct RowMutationDefaultsResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: bool,
}
impl RowMutationDefaultsResultSetProbe {
    /// 创建开放探针；参数为共享日志，返回结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self {
        Self {
            calls,
            closed: false,
        }
    }
    fn record(&self, method: &str) {
        self.calls
            .lock()
            .unwrap()
            .push(format!("physical:{method}"));
    }
}
druid_rdbc::impl_standard_wrapper!(RowMutationDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for RowMutationDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "row_mutation_defaults_next",
        })
    }
    fn insert_row(&mut self) -> Result<(), RdbcError> {
        self.record("insert_row");
        Ok(())
    }
    fn update_row(&mut self) -> Result<(), RdbcError> {
        self.record("update_row");
        Ok(())
    }
    fn delete_row(&mut self) -> Result<(), RdbcError> {
        self.record("delete_row");
        Ok(())
    }
    fn refresh_row(&mut self) -> Result<(), RdbcError> {
        self.record("refresh_row");
        Ok(())
    }
    fn cancel_row_updates(&mut self) -> Result<(), RdbcError> {
        self.record("cancel_row_updates");
        Ok(())
    }
    fn move_to_insert_row(&mut self) -> Result<(), RdbcError> {
        self.record("move_to_insert_row");
        Ok(())
    }
    fn move_to_current_row(&mut self) -> Result<(), RdbcError> {
        self.record("move_to_current_row");
        Ok(())
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        self.record("close");
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
