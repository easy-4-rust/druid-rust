//! URL/Ref/Array/RowId/SQLXML 平台对象完整资源契约。

use druid::core::{
    DruidError, PhysicalArray, PhysicalCharacterWriter, PhysicalRef, PhysicalResultSet,
    PhysicalSqlXml, PhysicalXmlResult, PhysicalXmlSource, RdbcArray, RdbcInputStream, RdbcObject,
    RdbcOutputStream, RdbcRef, RdbcResultSet, RdbcRowId, RdbcSqlXml, RdbcString, RdbcTargetType,
    RdbcTypeMap, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource, Value,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct TestResultSet {
    closed: AtomicBool,
}

impl PhysicalResultSet for TestResultSet {
    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

fn result_set() -> RdbcResultSet {
    RdbcResultSet::new(Arc::new(TestResultSet {
        closed: AtomicBool::new(false),
    }))
}

#[derive(Debug)]
struct TestArray {
    values: Vec<RdbcObject>,
    freed: AtomicBool,
}

impl TestArray {
    fn ensure_open(&self) -> Result<(), DruidError> {
        if self.freed.load(Ordering::Acquire) {
            Err(DruidError::DriverError("Array is freed".to_string()))
        } else {
            Ok(())
        }
    }

    fn range(&self, index: i64, count: i32) -> Result<Vec<RdbcObject>, DruidError> {
        self.ensure_open()?;
        let start = index
            .checked_sub(1)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| DruidError::DriverError("invalid Array index".to_string()))?;
        let count = usize::try_from(count)
            .map_err(|_| DruidError::DriverError("invalid Array count".to_string()))?;
        let end = start
            .checked_add(count)
            .ok_or_else(|| DruidError::DriverError("Array range overflow".to_string()))?;
        self.values
            .get(start..end)
            .map(<[RdbcObject]>::to_vec)
            .ok_or_else(|| DruidError::DriverError("invalid Array range".to_string()))
    }
}

impl PhysicalArray for TestArray {
    fn base_type_name(&self) -> Result<String, DruidError> {
        self.ensure_open()?;
        Ok("INTEGER".to_string())
    }

    fn base_type(&self) -> Result<i32, DruidError> {
        self.ensure_open()?;
        Ok(4)
    }

    fn values(&self) -> Result<Vec<RdbcObject>, DruidError> {
        self.ensure_open()?;
        Ok(self.values.clone())
    }

    fn values_with_type_map(&self, _type_map: &RdbcTypeMap) -> Result<Vec<RdbcObject>, DruidError> {
        self.values()
    }

    fn values_range(&self, index: i64, count: i32) -> Result<Vec<RdbcObject>, DruidError> {
        self.range(index, count)
    }

    fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        self.range(index, count)
    }

    fn result_set(&self) -> Result<RdbcResultSet, DruidError> {
        self.ensure_open()?;
        Ok(result_set())
    }

    fn result_set_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        self.result_set()
    }

    fn result_set_range(&self, index: i64, count: i32) -> Result<RdbcResultSet, DruidError> {
        self.range(index, count)?;
        Ok(result_set())
    }

    fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        self.result_set_range(index, count)
    }

    fn free(&self) -> Result<(), DruidError> {
        self.freed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_freed(&self) -> bool {
        self.freed.load(Ordering::Acquire)
    }
}

fn array() -> RdbcArray {
    RdbcArray::new(Arc::new(TestArray {
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

impl PhysicalRef for TestRef {
    fn base_type_name(&self) -> Result<String, DruidError> {
        Ok("schema.kind".to_string())
    }

    fn object(&self) -> Result<RdbcObject, DruidError> {
        Ok(self
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn object_with_type_map(&self, _type_map: &RdbcTypeMap) -> Result<RdbcObject, DruidError> {
        self.object()
    }

    fn set_object(&self, value: RdbcObject) -> Result<(), DruidError> {
        *self
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = value;
        Ok(())
    }
}

#[derive(Debug)]
struct TestCharacterWriter;

impl PhysicalCharacterWriter for TestCharacterWriter {
    fn write_utf16(&mut self, code_units: &[u16]) -> Result<usize, DruidError> {
        Ok(code_units.len())
    }

    fn flush(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
}

#[derive(Debug)]
struct TestXmlSource;
impl PhysicalXmlSource for TestXmlSource {}

#[derive(Debug)]
struct TestXmlResult;
impl PhysicalXmlResult for TestXmlResult {}

#[derive(Debug)]
struct TestSqlXml {
    value: Mutex<RdbcString>,
    freed: AtomicBool,
}

impl TestSqlXml {
    fn ensure_open(&self) -> Result<(), DruidError> {
        if self.freed.load(Ordering::Acquire) {
            Err(DruidError::DriverError("SQLXML is freed".to_string()))
        } else {
            Ok(())
        }
    }
}

impl PhysicalSqlXml for TestSqlXml {
    fn free(&self) -> Result<(), DruidError> {
        self.freed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_freed(&self) -> bool {
        self.freed.load(Ordering::Acquire)
    }

    fn binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.ensure_open()?;
        Ok(RdbcInputStream::from_bytes(
            self.string()?.to_rust_string()?.into_bytes(),
        ))
    }

    fn set_binary_stream(&self) -> Result<RdbcOutputStream, DruidError> {
        self.ensure_open()?;
        Ok(RdbcOutputStream::new(Vec::<u8>::new()))
    }

    fn character_stream(&self) -> Result<druid::core::RdbcReader, DruidError> {
        self.ensure_open()?;
        Ok(druid::core::RdbcReader::from_utf16(
            self.value
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_utf16()
                .to_vec(),
        ))
    }

    fn set_character_stream(&self) -> Result<RdbcWriter, DruidError> {
        self.ensure_open()?;
        Ok(RdbcWriter::new(TestCharacterWriter))
    }

    fn string(&self) -> Result<RdbcString, DruidError> {
        self.ensure_open()?;
        Ok(self
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn set_string(&self, value: &RdbcString) -> Result<(), DruidError> {
        self.ensure_open()?;
        *self
            .value
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = value.clone();
        Ok(())
    }

    fn source(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, DruidError> {
        self.ensure_open()?;
        Ok(RdbcXmlSource::new(Arc::new(TestXmlSource)))
    }

    fn result(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, DruidError> {
        self.ensure_open()?;
        Ok(RdbcXmlResult::new(Arc::new(TestXmlResult)))
    }
}

#[test]
fn array_and_ref_preserve_complete_rdbc_operations_and_identity() {
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
    assert!(format!("{value:?}").contains("freed: false"));
    assert_eq!(value.base_type_name().unwrap(), "INTEGER");
    assert_eq!(value.base_type().unwrap(), 4);
    assert_eq!(value.values().unwrap().len(), 3);
    assert_eq!(value.values_with_type_map(&type_map).unwrap().len(), 3);
    assert_eq!(
        value.values_range(2, 2).unwrap(),
        vec![
            RdbcObject::from(Value::Int(20)),
            RdbcObject::from(Value::Int(30))
        ]
    );
    assert_eq!(
        value.values_range_with_type_map(1, 1, &type_map).unwrap(),
        vec![RdbcObject::from(Value::Int(10))]
    );
    assert!(value.values_range(0, 1).is_err());
    for result_set in [
        value.result_set().unwrap(),
        value.result_set_with_type_map(&type_map).unwrap(),
        value.result_set_range(1, 1).unwrap(),
        value
            .result_set_range_with_type_map(2, 1, &type_map)
            .unwrap(),
    ] {
        assert!(!result_set.is_closed());
        assert!(format!("{result_set:?}").contains("closed: false"));
        result_set.close().unwrap();
        assert!(result_set.is_closed());
    }
    assert!(!value.is_freed());
    value.free().unwrap();
    assert!(value.is_freed());
    assert!(value.values().is_err());

    let reference = RdbcRef::new(Arc::new(TestRef {
        value: Mutex::new(RdbcObject::from(Value::Int(1))),
    }));
    assert_eq!(reference, reference.clone());
    assert!(format!("{reference:?}").contains("RdbcRef"));
    assert_eq!(reference.base_type_name().unwrap(), "schema.kind");
    assert_eq!(
        reference.object_with_type_map(&type_map).unwrap(),
        RdbcObject::from(Value::Int(1))
    );
    reference
        .set_object(RdbcObject::from(Value::String("next".to_string())))
        .unwrap();
    assert_eq!(
        reference.object().unwrap(),
        RdbcObject::from(Value::String("next".to_string()))
    );
}

#[test]
fn row_id_url_and_sql_xml_preserve_values_streams_and_resource_lifecycle() {
    let row_id = RdbcRowId::new(vec![0, 1, 255]);
    assert_eq!(row_id.bytes(), &[0, 1, 255]);
    assert_eq!(row_id, RdbcRowId::new(vec![0, 1, 255]));

    let url = druid::core::RdbcUrl::new("https://example.test/a?b=1#c");
    assert_eq!(url.external_form(), "https://example.test/a?b=1#c");
    assert_eq!(url, druid::core::RdbcUrl::from(url.external_form()));

    let xml = RdbcSqlXml::new(Arc::new(TestSqlXml {
        value: Mutex::new(RdbcString::from("<root>值</root>")),
        freed: AtomicBool::new(false),
    }));
    assert_eq!(xml, xml.clone());
    assert!(format!("{xml:?}").contains("freed: false"));
    assert_eq!(
        xml.binary_stream().unwrap().read_to_end().unwrap(),
        "<root>值</root>".as_bytes()
    );
    assert_eq!(
        xml.character_stream().unwrap().read_to_string().unwrap(),
        "<root>值</root>"
    );
    xml.set_binary_stream()
        .unwrap()
        .write(b"<binary/>")
        .unwrap();
    xml.set_character_stream()
        .unwrap()
        .write_str("<character/>")
        .unwrap();
    xml.set_string(&RdbcString::from_utf16(vec![0xD800]))
        .unwrap();
    assert_eq!(xml.string().unwrap().as_utf16(), &[0xD800]);
    let source = xml.source(&RdbcXmlRepresentationType::Dom).unwrap();
    assert_eq!(source, source.clone());
    assert!(format!("{source:?}").contains("RdbcXmlSource"));
    let result = xml.result(&RdbcXmlRepresentationType::Stream).unwrap();
    assert_eq!(result, result.clone());
    assert!(format!("{result:?}").contains("RdbcXmlResult"));
    xml.free().unwrap();
    assert!(xml.is_freed());
    assert!(xml.string().is_err());
}
