#![allow(dead_code, unused_imports)]
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    RdbcArrayAccess, RdbcBlob, RdbcBlobAccess, RdbcClob, RdbcClobAccess, RdbcInputStream,
    RdbcNClob, RdbcNClobAccess, RdbcOutputStream, RdbcReader, RdbcRefAccess, RdbcResourceAccess,
    RdbcSqlXmlAccess, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
    ResourceHandleFactory, ResourceOperations,
};
use druid_rdbc::types::{RdbcObject, RdbcString, RdbcTypeMap, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
/// 保留原始 Callable 测试资源逻辑。对应 Java: java.sql.CallableStatement。
#[derive(Debug)]
pub(crate) struct TestArrayAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for TestArrayAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::array()
    }
}

#[async_trait::async_trait]
impl RdbcArrayAccess for TestArrayAccess {
    async fn base_type_name(&self) -> Result<String, RdbcError> {
        Ok("INTEGER".to_string())
    }

    async fn base_type(&self) -> Result<i32, RdbcError> {
        Ok(4)
    }

    async fn values(&self) -> Result<Vec<RdbcObject>, RdbcError> {
        Ok(vec![RdbcObject::from(Value::Int(1))])
    }

    async fn values_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.values().await
    }

    async fn values_range(&self, _index: i64, _count: i32) -> Result<Vec<RdbcObject>, RdbcError> {
        self.values().await
    }

    async fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.values_range(index, count).await
    }

    async fn result_set(&self) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_array_result_set",
        })
    }

    async fn result_set_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.result_set().await
    }

    async fn result_set_range(
        &self,
        _index: i64,
        _count: i32,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.result_set().await
    }

    async fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.result_set_range(index, count).await
    }
}
