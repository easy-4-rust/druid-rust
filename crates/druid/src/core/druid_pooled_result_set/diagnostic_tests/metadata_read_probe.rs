use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::ResultSetMetaData;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::RdbcObject;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 保留原生metadata调用、共享句柄与驱动错误身份。对应 Java: java.sql.ResultSet。
pub(super) struct MetadataReadProbe {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) metadata: Arc<dyn ResultSetMetaData>,
    pub(super) fail: Arc<AtomicBool>,
    pub(super) closed: bool,
}
druid_rdbc::impl_standard_wrapper!(MetadataReadProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for MetadataReadProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "metadata_probe_next",
        })
    }
    fn object(&self, _column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "metadata_probe_object",
        })
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push("physical:meta_data".to_owned());
        if self.fail.load(Ordering::Acquire) {
            Err(RdbcError::DriverError("native metadata failure".to_owned()))
        } else {
            Ok(Arc::clone(&self.metadata))
        }
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
