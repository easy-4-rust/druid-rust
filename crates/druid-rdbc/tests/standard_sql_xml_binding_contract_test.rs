//! SQLXML 绑定前的驱动状态验证；对应 Java: SQLOutput#writeSQLXML。
use druid_rdbc::error::{RdbcError, SqlException, SqlExceptionCause};
use druid_rdbc::extensions::{BufferedSqlOutput, CharacterWriter, XmlResult, XmlSource};
use druid_rdbc::resource::{
    RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcSqlXml, RdbcWriter,
    RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource, ResourceHandleFactory,
    ResourceOperations, SqlInput, SqlOutput, SqlXml,
};
use druid_rdbc::types::{RdbcObject, RdbcString};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};

#[derive(Debug)]
struct Probe {
    binary: Mutex<Option<RdbcOutputStream>>,
    writer: Mutex<Option<RdbcWriter>>,
    result_pending: AtomicBool,
    reads: AtomicUsize,
    validations: AtomicUsize,
    error: Mutex<Option<SqlException>>,
    text: Mutex<RdbcString>,
}
impl Default for Probe {
    fn default() -> Self {
        Self {
            binary: Mutex::new(None),
            writer: Mutex::new(None),
            result_pending: AtomicBool::new(false),
            reads: AtomicUsize::new(0),
            validations: AtomicUsize::new(0),
            error: Mutex::new(None),
            text: Mutex::new(RdbcString::from("")),
        }
    }
}

#[derive(Debug)]
struct MemoryWriter(Arc<Probe>);
impl CharacterWriter for MemoryWriter {
    fn write_utf16(&mut self, units: &[u16]) -> Result<usize, RdbcError> {
        *self.0.text.lock().unwrap() = RdbcString::from_utf16(units.to_vec());
        Ok(units.len())
    }
    fn flush(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[derive(Debug)]
struct Representation;
impl XmlResult for Representation {}
impl XmlSource for Representation {}

#[derive(Debug)]
struct ReadyXml {
    probe: Arc<Probe>,
    freed: bool,
    // 驱动只要求Send；绑定验证不能额外要求Sync。
    local: std::cell::Cell<usize>,
}
impl ReadyXml {
    fn open(&self) -> Result<(), RdbcError> {
        if self.freed {
            Err(RdbcError::rdbc_resource_closed("SQLXML"))
        } else {
            Ok(())
        }
    }
    fn validate_binding(&mut self) -> Result<(), RdbcError> {
        self.open()?;
        self.local.set(self.local.get() + 1);
        self.probe.validations.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = self.probe.error.lock().unwrap().clone() {
            return Err(RdbcError::SqlException(Box::new(error)));
        }
        let binary_pending = self
            .probe
            .binary
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|s| !s.is_closed());
        let writer_pending = self
            .probe
            .writer
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|s| !s.is_closed());
        if binary_pending || writer_pending || self.probe.result_pending.load(Ordering::SeqCst) {
            return Err(RdbcError::SqlException(Box::new(SqlException::new(
                71,
                Some("HY010".into()),
                Some("XML output is not closed".into()),
            ))));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl SqlXml for ReadyXml {
    async fn validate_for_binding(&mut self) -> Result<(), RdbcError> {
        self.validate_binding()
    }
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::sql_xml()
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        self.freed = true;
        Ok(())
    }
    async fn binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        self.open()?;
        self.probe.reads.fetch_add(1, Ordering::SeqCst);
        Ok(RdbcInputStream::from_bytes(b"<ready/>".to_vec()))
    }
    async fn set_binary_stream(&mut self) -> Result<RdbcOutputStream, RdbcError> {
        self.open()?;
        let writer = RdbcOutputStream::new(std::io::sink());
        *self.probe.binary.lock().unwrap() = Some(writer.clone());
        Ok(writer)
    }
    async fn character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        self.open()?;
        self.probe.reads.fetch_add(1, Ordering::SeqCst);
        Ok(RdbcReader::from_string("<ready/>"))
    }
    async fn set_character_stream(&mut self) -> Result<RdbcWriter, RdbcError> {
        self.open()?;
        let writer = RdbcWriter::new(MemoryWriter(self.probe.clone()));
        *self.probe.writer.lock().unwrap() = Some(writer.clone());
        Ok(writer)
    }
    async fn string(&mut self) -> Result<RdbcString, RdbcError> {
        self.open()?;
        self.probe.reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.probe.text.lock().unwrap().clone())
    }
    async fn set_string(&mut self, value: &RdbcString) -> Result<(), RdbcError> {
        self.open()?;
        *self.probe.text.lock().unwrap() = value.clone();
        Ok(())
    }
    async fn source(
        &mut self,
        _kind: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlSource, RdbcError> {
        self.open()?;
        self.probe.reads.fetch_add(1, Ordering::SeqCst);
        Ok(RdbcXmlSource::new(Arc::new(Representation)))
    }
    async fn result(
        &mut self,
        _kind: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlResult, RdbcError> {
        self.open()?;
        self.probe.result_pending.store(true, Ordering::SeqCst);
        Ok(RdbcXmlResult::new(Arc::new(Representation)))
    }
}

#[derive(Debug)]
struct LegacyXml(ReadyXml);
#[async_trait::async_trait]
impl SqlXml for LegacyXml {
    fn capabilities(&self) -> ResourceOperations {
        self.0.capabilities()
    }
    async fn free(&mut self) -> Result<(), RdbcError> {
        self.0.free().await
    }
    async fn binary_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        self.0.binary_stream().await
    }
    async fn set_binary_stream(&mut self) -> Result<RdbcOutputStream, RdbcError> {
        self.0.set_binary_stream().await
    }
    async fn character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        self.0.character_stream().await
    }
    async fn set_character_stream(&mut self) -> Result<RdbcWriter, RdbcError> {
        self.0.set_character_stream().await
    }
    async fn string(&mut self) -> Result<RdbcString, RdbcError> {
        self.0.string().await
    }
    async fn set_string(&mut self, value: &RdbcString) -> Result<(), RdbcError> {
        self.0.set_string(value).await
    }
    async fn source(
        &mut self,
        kind: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlSource, RdbcError> {
        self.0.source(kind).await
    }
    async fn result(
        &mut self,
        kind: Option<&RdbcXmlRepresentationType>,
    ) -> Result<RdbcXmlResult, RdbcError> {
        self.0.result(kind).await
    }
}

fn xml(legacy: bool) -> (RdbcSqlXml, Arc<Probe>) {
    let probe = Arc::new(Probe::default());
    let driver = ReadyXml {
        probe: probe.clone(),
        freed: false,
        local: std::cell::Cell::new(0),
    };
    let driver: Box<dyn SqlXml> = if legacy {
        Box::new(LegacyXml(driver))
    } else {
        Box::new(driver)
    };
    (ResourceHandleFactory::sql_xml(driver), probe)
}
fn pending(error: RdbcError) {
    let RdbcError::SqlException(error) = error else {
        panic!("not SQL exception")
    };
    assert_eq!(error.error_code(), 71);
    assert_eq!(error.sql_state(), Some("HY010"));
    assert_eq!(error.message(), Some("XML output is not closed"));
}

#[tokio::test]
async fn open_binary_output_is_rejected_then_same_xml_binds_after_close() {
    let (value, probe) = xml(false);
    let writer = value.set_binary_stream().await.unwrap();
    writer.write(b"<ready/>").unwrap();
    let mut out = BufferedSqlOutput::new();
    pending(out.write_sql_xml(Some(value.clone())).await.unwrap_err());
    writer.close().unwrap();
    out.write_sql_xml(Some(value.clone())).await.unwrap();
    let mut input = out.into_input();
    assert_eq!(input.read_sql_xml().await.unwrap().unwrap(), value);
    assert!(input.read_object().await.is_err());
    assert_eq!(probe.validations.load(Ordering::SeqCst), 2);
    assert_eq!(probe.reads.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn open_character_writer_is_rejected_then_same_xml_binds_after_close() {
    let (value, _) = xml(false);
    let writer = value.set_character_stream().await.unwrap();
    writer.write_str("<ready/>").unwrap();
    let mut out = BufferedSqlOutput::new();
    pending(out.write_sql_xml(Some(value.clone())).await.unwrap_err());
    writer.close().unwrap();
    out.write_sql_xml(Some(value.clone())).await.unwrap();
    assert_eq!(
        out.into_input().read_sql_xml().await.unwrap().unwrap(),
        value
    );
}

#[tokio::test]
async fn unfinished_framework_result_is_rejected_until_driver_reports_completion() {
    let (value, probe) = xml(false);
    let result = value.result(None).await.unwrap();
    assert_eq!(result.clone(), result);
    let mut out = BufferedSqlOutput::new();
    pending(out.write_sql_xml(Some(value.clone())).await.unwrap_err());
    // 模拟适配器XML框架完成Result；标准层不猜测框架私有状态。
    probe.result_pending.store(false, Ordering::SeqCst);
    out.write_sql_xml(Some(value.clone())).await.unwrap();
    assert_eq!(
        out.into_input().read_sql_xml().await.unwrap().unwrap(),
        value
    );
}

#[tokio::test]
async fn xml_processing_error_preserves_cause_and_does_not_publish_an_attribute() {
    let (value, probe) = xml(false);
    let expected = SqlException::new(
        73,
        Some("2200N".into()),
        Some("invalid XML at offset 17".into()),
    )
    .with_cause(SqlExceptionCause::ClassName(
        "org.xml.sax.SAXParseException".into(),
    ));
    *probe.error.lock().unwrap() = Some(expected.clone());
    let mut out = BufferedSqlOutput::new();
    let RdbcError::SqlException(error) = out.write_sql_xml(Some(value)).await.unwrap_err() else {
        panic!("lost XML error")
    };
    assert_eq!(*error, expected);
    out.write_int(7).await.unwrap();
    let mut input = out.into_input();
    assert_eq!(input.read_int().await.unwrap(), 7);
    assert!(input.read_object().await.is_err());
}

#[tokio::test]
async fn freed_xml_is_rejected_before_driver_validation_or_attribute_append() {
    let (value, probe) = xml(false);
    value.free().await.unwrap();
    let mut out = BufferedSqlOutput::new();
    let error = out.write_sql_xml(Some(value)).await.unwrap_err();
    assert_eq!(error.class_name(), "java.sql.SQLException");
    assert_eq!(probe.validations.load(Ordering::SeqCst), 0);
    assert!(out.into_input().read_object().await.is_err());
}

#[tokio::test]
async fn ready_xml_preserves_identity_and_does_not_consume_its_first_read() {
    let (value, probe) = xml(false);
    value
        .set_string(&RdbcString::from("<ready/>"))
        .await
        .unwrap();
    let mut out = BufferedSqlOutput::new();
    out.write_sql_xml(Some(value.clone())).await.unwrap();
    assert_eq!(probe.validations.load(Ordering::SeqCst), 1);
    assert_eq!(probe.reads.load(Ordering::SeqCst), 0);
    let copied = out.into_input().read_sql_xml().await.unwrap().unwrap();
    assert_eq!(copied, value);
    assert_eq!(
        copied.get_string().await.unwrap(),
        RdbcString::from("<ready/>")
    );
    assert_eq!(probe.reads.load(Ordering::SeqCst), 1);
    assert!(value.get_string().await.is_err());
}

#[tokio::test]
async fn unknown_driver_binding_state_fails_closed_without_guessing_from_a_read() {
    let (value, probe) = xml(true);
    let mut out = BufferedSqlOutput::new();
    let error = out.write_sql_xml(Some(value.clone())).await.unwrap_err();
    assert_eq!(
        error.class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert_eq!(probe.reads.load(Ordering::SeqCst), 0);
    assert!(out.into_input().read_object().await.is_err());
    assert!(value.get_string().await.is_ok());
}

#[tokio::test]
async fn null_xml_appends_exactly_one_sql_null() {
    let mut out = BufferedSqlOutput::new();
    out.write_sql_xml(None).await.unwrap();
    let mut input = out.into_input();
    assert!(matches!(
        input.read_object().await.unwrap(),
        RdbcObject::Scalar(druid_rdbc::types::Value::Null)
    ));
    assert!(input.was_null().unwrap());
    assert!(input.read_object().await.is_err());
}
