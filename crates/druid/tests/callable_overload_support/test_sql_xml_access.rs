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
pub(crate) struct TestSqlXmlAccess;

#[async_trait::async_trait]
impl SqlXml for TestSqlXmlAccess {
    // 原夹具无外部资源，保留原默认free的幂等成功语义。
    async fn free(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::sql_xml()
    }

    async fn binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        Ok(RdbcInputStream::from_bytes(b"<x/>".to_vec()))
    }

    async fn set_binary_stream(&mut self) -> Result<RdbcOutputStream, RdbcError> {
        Ok(RdbcOutputStream::new(Vec::<u8>::new()))
    }

    async fn character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        Ok(RdbcReader::from_string("<x/>"))
    }

    async fn set_character_stream(&mut self) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_sql_xml_writer",
        })
    }

    async fn string(&mut self) -> Result<RdbcString, RdbcError> {
        Ok(RdbcString::from("<x/>"))
    }

    async fn set_string(&mut self, _value: &RdbcString) -> Result<(), RdbcError> {
        Ok(())
    }

    async fn source(
        &mut self,
        _representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlSource, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_sql_xml_source",
        })
    }

    async fn result(
        &mut self,
        _representation: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlResult, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_sql_xml_result",
        })
    }
}
