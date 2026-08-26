//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! URL/Ref/Array/RowId/SQLXML 平台对象完整资源契约。

use druid::core::{
    CharacterWriter, RdbcArray, RdbcError, RdbcInputStream, RdbcObject, RdbcOutputStream,
    RdbcRowId, RdbcString, RdbcTargetType, RdbcTypeMap, RdbcWriter, RdbcXmlRepresentationType,
    RdbcXmlResult, RdbcXmlSource, ResultSet, Value, XmlResult, XmlSource,
};
use druid::core::{
    RdbcArrayAccess, RdbcRefAccess, RdbcResourceAccess, RdbcSqlXmlAccess, ResourceHandleFactory,
    ResourceOperations,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct TestResultSet {
    closed: AtomicBool,
}

druid_rdbc::impl_standard_wrapper!(TestResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for TestResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }
}

fn result_set() -> Box<dyn ResultSet> {
    Box::new(TestResultSet {
        closed: AtomicBool::new(false),
    })
}

#[derive(Debug)]
struct TestArray {
    values: Vec<RdbcObject>,
    freed: AtomicBool,
}

impl TestArray {
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.freed.load(Ordering::Acquire) {
            Err(RdbcError::DriverError("Array is freed".to_string()))
        } else {
            Ok(())
        }
    }

    fn range(&self, index: i64, count: i32) -> Result<Vec<RdbcObject>, RdbcError> {
        self.ensure_open()?;
        let start = index
            .checked_sub(1)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| RdbcError::DriverError("invalid Array index".to_string()))?;
        let count = usize::try_from(count)
            .map_err(|_| RdbcError::DriverError("invalid Array count".to_string()))?;
        let end = start
            .checked_add(count)
            .ok_or_else(|| RdbcError::DriverError("Array range overflow".to_string()))?;
        self.values
            .get(start..end)
            .map(<[RdbcObject]>::to_vec)
            .ok_or_else(|| RdbcError::DriverError("invalid Array range".to_string()))
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for TestArray {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::array()
    }

    async fn free(&self) -> Result<(), RdbcError> {
        self.freed.store(true, Ordering::Release);
        Ok(())
    }
}

#[async_trait::async_trait]
impl RdbcArrayAccess for TestArray {
    async fn base_type_name(&self) -> Result<String, RdbcError> {
        self.ensure_open()?;
        Ok("INTEGER".to_string())
    }

    async fn base_type(&self) -> Result<i32, RdbcError> {
        self.ensure_open()?;
        Ok(4)
    }

    async fn values(&self) -> Result<Vec<RdbcObject>, RdbcError> {
        self.ensure_open()?;
        Ok(self.values.clone())
    }

    async fn values_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.values().await
    }

    async fn values_range(&self, index: i64, count: i32) -> Result<Vec<RdbcObject>, RdbcError> {
        self.range(index, count)
    }

    async fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, RdbcError> {
        self.range(index, count)
    }

    async fn result_set(&self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.ensure_open()?;
        Ok(result_set())
    }

    async fn result_set_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.result_set().await
    }

    async fn result_set_range(
        &self,
        index: i64,
        count: i32,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.range(index, count)?;
        Ok(result_set())
    }

    async fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.result_set_range(index, count).await
    }
}

fn array() -> RdbcArray {
    ResourceHandleFactory::array(Arc::new(TestArray {
        values: vec![
            RdbcObject::from(Value::Int(10)),
            RdbcObject::from(Value::Int(20)),
            RdbcObject::from(Value::Int(30)),
        ],
        freed: AtomicBool::new(false),
    }))
}

#[derive(Debug)]
struct TestRef {
    value: Mutex<RdbcObject>,
}

#[async_trait::async_trait]
impl RdbcResourceAccess for TestRef {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::reference()
    }
}

#[async_trait::async_trait]
impl RdbcRefAccess for TestRef {
    async fn base_type_name(&self) -> Result<String, RdbcError> {
        Ok("schema.kind".to_string())
    }

    async fn object(&self) -> Result<RdbcObject, RdbcError> {
        Ok(self
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    async fn object_with_type_map(&self, _type_map: &RdbcTypeMap) -> Result<RdbcObject, RdbcError> {
        self.object().await
    }

    async fn set_object(&self, value: RdbcObject) -> Result<(), RdbcError> {
        *self
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = value;
        Ok(())
    }
}

#[derive(Debug)]
struct TestCharacterWriter;

impl CharacterWriter for TestCharacterWriter {
    fn write_utf16(&mut self, code_units: &[u16]) -> Result<usize, RdbcError> {
        Ok(code_units.len())
    }

    fn flush(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[derive(Debug)]
struct TestXmlSource;
impl XmlSource for TestXmlSource {}

#[derive(Debug)]
struct TestXmlResult;
impl XmlResult for TestXmlResult {}

#[derive(Debug)]
struct TestSqlXml {
    value: Mutex<RdbcString>,
    freed: AtomicBool,
}

impl TestSqlXml {
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.freed.load(Ordering::Acquire) {
            Err(RdbcError::DriverError("SQLXML is freed".to_string()))
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for TestSqlXml {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::sql_xml()
    }

    async fn free(&self) -> Result<(), RdbcError> {
        self.freed.store(true, Ordering::Release);
        Ok(())
    }
}

#[async_trait::async_trait]
impl RdbcSqlXmlAccess for TestSqlXml {
    async fn binary_stream(&self) -> Result<RdbcInputStream, RdbcError> {
        self.ensure_open()?;
        Ok(RdbcInputStream::from_bytes(
            self.string().await?.to_rust_string()?.into_bytes(),
        ))
    }

    async fn set_binary_stream(&self) -> Result<RdbcOutputStream, RdbcError> {
        self.ensure_open()?;
        Ok(RdbcOutputStream::new(Vec::<u8>::new()))
    }

    async fn character_stream(&self) -> Result<druid::core::RdbcReader, RdbcError> {
        self.ensure_open()?;
        Ok(druid::core::RdbcReader::from_utf16(
            self.value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_utf16()
                .to_vec(),
        ))
    }

    async fn set_character_stream(&self) -> Result<RdbcWriter, RdbcError> {
        self.ensure_open()?;
        Ok(RdbcWriter::new(TestCharacterWriter))
    }

    async fn string(&self) -> Result<RdbcString, RdbcError> {
        self.ensure_open()?;
        Ok(self
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    async fn set_string(&self, value: &RdbcString) -> Result<(), RdbcError> {
        self.ensure_open()?;
        *self
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = value.clone();
        Ok(())
    }

    async fn source(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, RdbcError> {
        self.ensure_open()?;
        Ok(RdbcXmlSource::new(Arc::new(TestXmlSource)))
    }

    async fn result(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, RdbcError> {
        self.ensure_open()?;
        Ok(RdbcXmlResult::new(Arc::new(TestXmlResult)))
    }
}

#[tokio::test]
async fn task6_rdbc_extended_types_test_array_and_ref_preserve_complete_rdbc_operations_and_identity(
) {
    let mut type_map = RdbcTypeMap::new();
    assert!(type_map.is_empty());
    assert_eq!(type_map.insert("schema.kind", RdbcTargetType::String), None);
    assert_eq!(type_map.len(), 1);
    assert_eq!(type_map.get("schema.kind"), Some(&RdbcTargetType::String));
    assert_eq!(type_map.mappings().len(), 1);
    let copied_map = RdbcTypeMap::from_mappings(type_map.mappings().clone());
    assert_eq!(copied_map, type_map);

    let value = array();
    assert_eq!(value, value.clone());
    assert_ne!(value, array());
    assert!(format!("{value:?}").contains("state: Open"));
    assert_eq!(value.base_type_name().await.unwrap(), "INTEGER");
    assert_eq!(value.base_type().await.unwrap(), 4);
    assert_eq!(value.values().await.unwrap().len(), 3);
    assert_eq!(
        value.values_with_type_map(&type_map).await.unwrap().len(),
        3
    );
    assert_eq!(
        value.values_range(2, 2).await.unwrap(),
        vec![
            RdbcObject::from(Value::Int(20)),
            RdbcObject::from(Value::Int(30))
        ]
    );
    assert_eq!(
        value
            .values_range_with_type_map(1, 1, &type_map)
            .await
            .unwrap(),
        vec![RdbcObject::from(Value::Int(10))]
    );
    assert!(value.values_range(0, 1).await.is_err());
    for mut result_set in [
        value.result_set().await.unwrap(),
        value.result_set_with_type_map(&type_map).await.unwrap(),
        value.result_set_range(1, 1).await.unwrap(),
        value
            .result_set_range_with_type_map(2, 1, &type_map)
            .await
            .unwrap(),
    ] {
        assert!(!result_set.is_closed().unwrap());
        assert!(matches!(
            Wrapper::unwrap(&*result_set, Some(TypeId::of::<dyn ResultSet>())),
            Ok(StandardUnwrapped::ResultSet(_))
        ));
        result_set.close().await.unwrap();
        assert!(result_set.is_closed().unwrap());
    }
    assert!(!value.is_freed());
    value.free().await.unwrap();
    assert!(value.is_freed());
    assert!(value.values().await.is_err());

    let reference = ResourceHandleFactory::reference(Arc::new(TestRef {
        value: Mutex::new(RdbcObject::from(Value::Int(1))),
    }));
    assert_eq!(reference, reference.clone());
    assert!(format!("{reference:?}").contains("RdbcRef"));
    assert_eq!(reference.base_type_name().await.unwrap(), "schema.kind");
    assert_eq!(
        reference.object_with_type_map(&type_map).await.unwrap(),
        RdbcObject::from(Value::Int(1))
    );
    reference
        .set_object(RdbcObject::from(Value::String("next".to_string())))
        .await
        .unwrap();
    assert_eq!(
        reference.object().await.unwrap(),
        RdbcObject::from(Value::String("next".to_string()))
    );
}

#[tokio::test]
async fn task6_rdbc_extended_types_test_row_id_url_and_sql_xml_preserve_values_streams_and_resource_lifecycle(
) {
    let row_id = RdbcRowId::new(vec![0, 1, 255]);
    assert_eq!(row_id.bytes(), &[0, 1, 255]);
    assert_eq!(row_id, RdbcRowId::new(vec![0, 1, 255]));

    let url = druid::core::RdbcUrl::new("https://example.test/a?b=1#c");
    assert_eq!(url.external_form(), "https://example.test/a?b=1#c");
    assert_eq!(url, druid::core::RdbcUrl::from(url.external_form()));

    let xml = ResourceHandleFactory::sql_xml(Arc::new(TestSqlXml {
        value: Mutex::new(RdbcString::from("<root>值</root>")),
        freed: AtomicBool::new(false),
    }));
    assert_eq!(xml, xml.clone());
    assert!(format!("{xml:?}").contains("state: Open"));
    assert_eq!(
        xml.binary_stream().await.unwrap().read_to_end().unwrap(),
        "<root>值</root>".as_bytes()
    );
    assert_eq!(
        xml.character_stream()
            .await
            .unwrap()
            .read_to_string()
            .unwrap(),
        "<root>值</root>"
    );
    xml.set_binary_stream()
        .await
        .unwrap()
        .write(b"<binary/>")
        .unwrap();
    xml.set_character_stream()
        .await
        .unwrap()
        .write_str("<character/>")
        .unwrap();
    xml.set_string(&RdbcString::from_utf16(vec![0xD800]))
        .await
        .unwrap();
    assert_eq!(xml.string().await.unwrap().as_utf16(), &[0xD800]);
    let source = xml.source(&RdbcXmlRepresentationType::Dom).await.unwrap();
    assert_eq!(source, source.clone());
    assert!(format!("{source:?}").contains("RdbcXmlSource"));
    let result = xml
        .result(&RdbcXmlRepresentationType::Stream)
        .await
        .unwrap();
    assert_eq!(result, result.clone());
    assert!(format!("{result:?}").contains("RdbcXmlResult"));
    xml.free().await.unwrap();
    assert!(xml.is_freed());
    assert!(xml.string().await.is_err());
}
