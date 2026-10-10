#![allow(dead_code, unused_imports)]
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    Array, Blob, Clob, NClob, RdbcBlob, RdbcClob, RdbcInputStream, RdbcNClob, RdbcOutputStream,
    RdbcReader, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource, Ref,
    ResourceHandleFactory, ResourceOperations, SqlXml,
};
use druid_rdbc::types::{RdbcObject, RdbcString, RdbcTypeMap, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
/// 保留原始 Callable 测试资源逻辑。对应 Java: java.sql.CallableStatement。
#[derive(Debug)]
pub(crate) struct TestRefAccess;

#[async_trait::async_trait]
impl Ref for TestRefAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::reference()
    }

    async fn base_type_name(&mut self) -> Result<String, RdbcError> {
        Ok("schema.kind".to_string())
    }

    async fn object(&mut self) -> Result<RdbcObject, RdbcError> {
        Ok(RdbcObject::from(Value::String("ref-value".to_string())))
    }

    async fn object_with_type_map(
        &mut self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, RdbcError> {
        self.object().await
    }

    async fn set_object(&mut self, _value: RdbcObject) -> Result<(), RdbcError> {
        Ok(())
    }
}
