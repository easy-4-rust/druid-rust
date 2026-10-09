//! 完整对象过滤默认链的原生探针候选，不扁平化返回对象。
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap};
use std::sync::{Arc, Mutex};

/// 原生读取探针，保留六种入口、借用参数和返回句柄身份。
/// 对应 Java: java.sql.ResultSet#getObject。
pub(super) struct ObjectReadProbe {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) arguments: Arc<Mutex<Vec<usize>>>,
    pub(super) object: RdbcObject,
    pub(super) closed: bool,
}

druid_rdbc::impl_standard_wrapper!(ObjectReadProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ObjectReadProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "object_probe_next",
        })
    }
    fn object(&self, column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        self.read(format!("raw:ordinary:{}", column.get()))
    }
    fn object_by_label(&self, label: &str) -> Result<RdbcObject, RdbcError> {
        self.read(format!("raw:ordinary_label:{label}"))
    }
    fn object_with_type_map(
        &self,
        column: ColumnIndex,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.record_map(map);
        self.read(format!("raw:map:{}:{}", column.get(), map.is_some()))
    }
    fn object_with_type_map_by_label(
        &self,
        label: &str,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.record_map(map);
        self.read(format!("raw:map_label:{label}:{}", map.is_some()))
    }
    fn object_as(
        &self,
        column: ColumnIndex,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.arguments
            .lock()
            .unwrap()
            .push(std::ptr::from_ref(target) as usize);
        self.read(format!("raw:typed:{}:{target:?}", column.get()))
    }
    fn object_as_by_label(
        &self,
        label: &str,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.arguments
            .lock()
            .unwrap()
            .push(std::ptr::from_ref(target) as usize);
        self.read(format!("raw:typed_label:{label}:{target:?}"))
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("unexpected_find_column:{label}"));
        ColumnIndex::new(23)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![])))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
impl ObjectReadProbe {
    fn read(&self, call: String) -> Result<RdbcObject, RdbcError> {
        if self.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let failed = call.starts_with("raw:typed_label:fail:");
        self.calls.lock().unwrap().push(call);
        if failed {
            return Err(RdbcError::DriverError("object probe failure".to_owned()));
        }
        Ok(self.object.clone())
    }
    fn record_map(&self, map: Option<&RdbcTypeMap>) {
        if let Some(map) = map {
            self.arguments
                .lock()
                .unwrap()
                .push(std::ptr::from_ref(map) as usize);
        }
    }
}
