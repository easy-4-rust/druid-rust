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
pub(crate) struct TestRefAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for TestRefAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::reference()
    }
}

#[async_trait::async_trait]
impl RdbcRefAccess for TestRefAccess {
    async fn base_type_name(&self) -> Result<String, RdbcError> {
        Ok("schema.kind".to_string())
    }

    async fn object(&self) -> Result<RdbcObject, RdbcError> {
        Ok(RdbcObject::from(Value::String("ref-value".to_string())))
    }

    async fn object_with_type_map(&self, _type_map: &RdbcTypeMap) -> Result<RdbcObject, RdbcError> {
        self.object().await
    }

    async fn set_object(&self, _value: RdbcObject) -> Result<(), RdbcError> {
        Ok(())
    }
}
