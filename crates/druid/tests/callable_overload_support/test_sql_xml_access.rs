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
pub(crate) struct TestSqlXmlAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for TestSqlXmlAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::sql_xml()
    }
}

#[async_trait::async_trait]
impl RdbcSqlXmlAccess for TestSqlXmlAccess {
    async fn binary_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        Ok(RdbcInputStream::from_bytes(b"<x/>".to_vec()))
    }

    async fn set_binary_stream(&self) -> Result<RdbcOutputStream, RdbcError> {
        Ok(RdbcOutputStream::new(Vec::<u8>::new()))
    }

    async fn character_stream(&self) -> Result<RdbcReader, RdbcError> {
        Ok(RdbcReader::from_string("<x/>"))
    }

    async fn set_character_stream(&self) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_sql_xml_writer",
        })
    }

    async fn string(&self) -> Result<RdbcString, RdbcError> {
        Ok(RdbcString::from("<x/>"))
    }

    async fn set_string(&self, _value: &RdbcString) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn source(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_sql_xml_source",
        })
    }

    async fn result(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_sql_xml_result",
        })
    }
}
