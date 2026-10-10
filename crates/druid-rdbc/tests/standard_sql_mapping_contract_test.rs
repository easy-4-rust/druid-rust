//! 固定 JDBC 4.2 SQLData/Input/Output 的独立标准行为合同。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{BufferedSqlInput, BufferedSqlOutput};
use druid_rdbc::resource::{RdbcInputStream, RdbcReader, SqlData, SqlInput, SqlOutput};
use druid_rdbc::types::{JdbcType, RdbcObject, RdbcTargetType, Value};

#[derive(Default)]
struct MappedRecord {
    type_name: String,
    number: i32,
    label: Option<String>,
}

#[async_trait::async_trait]
impl SqlData for MappedRecord {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok(self.type_name.clone())
    }
    async fn read_sql(
        &mut self,
        input: &mut dyn SqlInput,
        type_name: &str,
    ) -> Result<(), RdbcError> {
        self.type_name = type_name.to_owned();
        self.number = input.read_int().await?;
        self.label = input.read_string().await?;
        Ok(())
    }
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        output.write_int(self.number).await?;
        output.write_string(self.label.clone()).await
    }
}

#[tokio::test]
async fn every_standard_input_reader_distinguishes_sql_null() {
    let mut input = BufferedSqlInput::new(vec![RdbcObject::Scalar(Value::Null); 26]);
    macro_rules! primitive {
        ($method:ident, $zero:expr) => {
            assert_eq!(input.$method().await.unwrap(), $zero);
            assert!(input.was_null().unwrap());
        };
    }
    macro_rules! nullable {
        ($method:ident) => {
            assert!(input.$method().await.unwrap().is_none());
            assert!(input.was_null().unwrap());
        };
    }
    nullable!(read_string);
    primitive!(read_boolean, false);
    primitive!(read_byte, 0);
    primitive!(read_short, 0);
    primitive!(read_int, 0);
    primitive!(read_long, 0);
    primitive!(read_float, 0.0);
    primitive!(read_double, 0.0);
    nullable!(read_big_decimal);
    nullable!(read_bytes);
    nullable!(read_date);
    nullable!(read_time);
    nullable!(read_timestamp);
    nullable!(read_character_stream);
    nullable!(read_ascii_stream);
    nullable!(read_binary_stream);
    assert!(input.read_object().await.unwrap().is_null());
    assert!(input.was_null().unwrap());
    nullable!(read_ref);
    nullable!(read_blob);
    nullable!(read_clob);
    nullable!(read_array);
    nullable!(read_url);
    nullable!(read_n_clob);
    nullable!(read_n_string);
    nullable!(read_sql_xml);
    nullable!(read_row_id);
    assert!(
        input.read_object().await.is_err(),
        "every reader must consume exactly one attribute"
    );
}

#[tokio::test]
async fn every_standard_output_writer_retains_attribute_order_and_nulls() {
    let mut output = BufferedSqlOutput::new();
    output.write_string(None).await.unwrap();
    output.write_boolean(true).await.unwrap();
    output.write_byte(i8::MIN).await.unwrap();
    output.write_short(i16::MAX).await.unwrap();
    output.write_int(i32::MIN).await.unwrap();
    output.write_long(i64::MAX).await.unwrap();
    output.write_float(1.5).await.unwrap();
    output.write_double(-2.5).await.unwrap();
    output.write_big_decimal(None).await.unwrap();
    output.write_bytes(None).await.unwrap();
    output.write_date(None).await.unwrap();
    output.write_time(None).await.unwrap();
    output.write_timestamp(None).await.unwrap();
    output.write_character_stream(None).await.unwrap();
    output.write_ascii_stream(None).await.unwrap();
    output.write_binary_stream(None).await.unwrap();
    output.write_object(None).await.unwrap();
    output.write_ref(None).await.unwrap();
    output.write_blob(None).await.unwrap();
    output.write_clob(None).await.unwrap();
    output.write_struct(None).await.unwrap();
    output.write_array(None).await.unwrap();
    output.write_url(None).await.unwrap();
    output.write_n_string(None).await.unwrap();
    output.write_n_clob(None).await.unwrap();
    output.write_row_id(None).await.unwrap();
    output.write_sql_xml(None).await.unwrap();
    let mut input = output.into_input();
    assert!(input.read_string().await.unwrap().is_none());
    assert!(input.was_null().unwrap());
    assert!(input.read_boolean().await.unwrap());
    assert!(!input.was_null().unwrap());
    assert_eq!(input.read_byte().await.unwrap(), i8::MIN);
    assert_eq!(input.read_short().await.unwrap(), i16::MAX);
    assert_eq!(input.read_int().await.unwrap(), i32::MIN);
    assert_eq!(input.read_long().await.unwrap(), i64::MAX);
    assert_eq!(input.read_float().await.unwrap(), 1.5);
    assert_eq!(input.read_double().await.unwrap(), -2.5);
    for _ in 0..19 {
        assert!(input.read_object().await.unwrap().is_null());
        assert!(input.was_null().unwrap());
    }
    assert!(input.read_object().await.is_err());
}

#[tokio::test]
async fn external_sql_data_reads_and_writes_the_declared_udt_order() {
    let mut input = BufferedSqlInput::new(vec![Value::Int(41).into(), Value::Null.into()]);
    let mut object = MappedRecord::default();
    let data: &mut dyn SqlData = &mut object;
    data.read_sql(&mut input, "app.record").await.unwrap();
    assert_eq!(data.sql_type_name().await.unwrap(), "app.record");
    let mut output = BufferedSqlOutput::new();
    data.write_sql(&mut output).await.unwrap();
    let mut round_trip = output.into_input();
    assert_eq!(round_trip.read_int().await.unwrap(), 41);
    assert!(!round_trip.was_null().unwrap());
    assert!(round_trip.read_string().await.unwrap().is_none());
    assert!(round_trip.was_null().unwrap());
    assert!(round_trip.read_object().await.is_err());
}

#[tokio::test]
async fn conversion_failure_consumes_only_the_offending_attribute_and_preserves_null_state() {
    let mut input = BufferedSqlInput::new(vec![
        Value::String("wrong".into()).into(),
        Value::Null.into(),
        Value::Int(7).into(),
    ]);
    assert!(input.read_int().await.is_err());
    assert!(!input.was_null().unwrap());
    assert!(input.read_string().await.unwrap().is_none());
    assert!(input.was_null().unwrap());
    assert_eq!(input.read_int().await.unwrap(), 7);
    assert!(!input.was_null().unwrap());
    assert!(input.read_int().await.is_err());
}

#[tokio::test]
async fn jdbc42_default_typed_overloads_reject_without_consuming_or_appending() {
    let mut input = BufferedSqlInput::new(vec![Value::String("retained".into()).into()]);
    let error = input
        .read_object_as(&RdbcTargetType::String)
        .await
        .unwrap_err();
    assert!(
        matches!(error, RdbcError::SqlException(ref e) if e.class_name() == "java.sql.SQLFeatureNotSupportedException")
    );
    assert_eq!(
        input.read_string().await.unwrap().as_deref(),
        Some("retained")
    );
    let mut output = BufferedSqlOutput::new();
    output.write_int(9).await.unwrap();
    let error = output
        .write_object_as(Some(Value::Int(10).into()), &JdbcType::Integer)
        .await
        .unwrap_err();
    assert!(
        matches!(error, RdbcError::SqlException(ref e) if e.class_name() == "java.sql.SQLFeatureNotSupportedException")
    );
    let mut input = output.into_input();
    assert_eq!(input.read_int().await.unwrap(), 9);
    assert!(input.read_int().await.is_err());
}

#[tokio::test]
async fn sql_stream_attributes_preserve_the_original_cursor_and_utf16_units() {
    let ascii = RdbcInputStream::from_bytes(b"abc".to_vec());
    let binary = RdbcInputStream::from_bytes(vec![0, 255, 128]);
    let text = RdbcReader::from_utf16(vec![0xD800, 0x0061, 0xDC00]);
    let mut output = BufferedSqlOutput::new();
    output
        .write_ascii_stream(Some(ascii.clone()))
        .await
        .unwrap();
    output
        .write_binary_stream(Some(binary.clone()))
        .await
        .unwrap();
    output
        .write_character_stream(Some(text.clone()))
        .await
        .unwrap();
    let mut one_byte = [0];
    assert_eq!(ascii.read(&mut one_byte).unwrap(), 1);
    assert_eq!(one_byte, [b'a']);
    let mut input = output.into_input();
    let read_ascii = input.read_ascii_stream().await.unwrap().unwrap();
    assert_eq!(read_ascii, ascii);
    assert_eq!(read_ascii.read_to_end().unwrap(), b"bc");
    let read_binary = input.read_binary_stream().await.unwrap().unwrap();
    assert_eq!(read_binary, binary);
    assert_eq!(read_binary.read_to_end().unwrap(), vec![0, 255, 128]);
    let reader = input.read_character_stream().await.unwrap().unwrap();
    assert_eq!(reader, text);
    assert_eq!(
        reader.read_to_end_utf16().unwrap(),
        vec![0xD800, 0x0061, 0xDC00]
    );
    assert!(text.read_to_end_utf16().unwrap().is_empty());
}

// SQLData 不应强制应用对象 Sync；驱动独占访问足以提供可迁移的 Send future。
struct SendOnlyData(std::cell::Cell<i32>);
#[async_trait::async_trait]
impl SqlData for SendOnlyData {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok("app.send_only".into())
    }
    async fn read_sql(&mut self, input: &mut dyn SqlInput, _name: &str) -> Result<(), RdbcError> {
        let value = input.read_int().await?;
        self.0.set(value);
        Ok(())
    }
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        output.write_int(self.0.get()).await
    }
}
#[tokio::test]
async fn custom_sql_data_can_be_send_without_sync_on_a_spawned_task() {
    tokio::spawn(async {
        let mut object = SendOnlyData(std::cell::Cell::new(0));
        let data: &mut dyn SqlData = &mut object;
        let mut input = BufferedSqlInput::new(vec![Value::Int(73).into()]);
        data.read_sql(&mut input, "app.send_only").await.unwrap();
        assert_eq!(data.sql_type_name().await.unwrap(), "app.send_only");
        let mut output = BufferedSqlOutput::new();
        data.write_sql(&mut output).await.unwrap();
        assert_eq!(output.into_input().read_int().await.unwrap(), 73);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn output_custom_sql_data_is_one_structured_attribute_with_its_type_name() {
    let mut data = MappedRecord {
        type_name: "app.record".into(),
        number: 41,
        label: None,
    };
    let mut output = BufferedSqlOutput::new();
    output.write_object(Some(&mut data)).await.unwrap();
    output.write_int(99).await.unwrap();
    let mut input = output.into_input();
    let RdbcObject::Struct(value) = input.read_object().await.unwrap() else {
        panic!("custom SQLData must retain one UDT boundary")
    };
    assert_eq!(value.sql_type_name().await.unwrap(), "app.record");
    let mut attributes = BufferedSqlInput::new(value.attributes().await.unwrap());
    assert_eq!(attributes.read_int().await.unwrap(), 41);
    assert!(attributes.read_string().await.unwrap().is_none());
    assert!(attributes.was_null().unwrap());
    assert!(attributes.read_object().await.is_err());
    assert_eq!(input.read_int().await.unwrap(), 99);
    assert!(input.read_object().await.is_err());
}

struct FailingMappedData {
    fail_name: bool,
}
#[async_trait::async_trait]
impl SqlData for FailingMappedData {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        if self.fail_name {
            Err(RdbcError::DriverError("original typename failure".into()))
        } else {
            Ok("app.failure".into())
        }
    }
    async fn read_sql(&mut self, _input: &mut dyn SqlInput, _name: &str) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("test-read"))
    }
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        output.write_int(1).await?;
        Err(RdbcError::SqlException(Box::new(
            druid_rdbc::error::SqlException::new(
                73,
                Some("22000".into()),
                Some("original write failure".into()),
            ),
        )))
    }
}
#[tokio::test]
async fn failed_custom_mapping_preserves_the_original_error_without_partial_outer_attribute() {
    for fail_name in [false, true] {
        let mut data = FailingMappedData { fail_name };
        let mut output = BufferedSqlOutput::new();
        let expected = if fail_name {
            RdbcError::DriverError("original typename failure".into())
        } else {
            RdbcError::SqlException(Box::new(druid_rdbc::error::SqlException::new(
                73,
                Some("22000".into()),
                Some("original write failure".into()),
            )))
        };
        assert_eq!(
            output.write_object(Some(&mut data)).await.unwrap_err(),
            expected
        );
        output.write_int(7).await.unwrap();
        let mut input = output.into_input();
        assert_eq!(input.read_int().await.unwrap(), 7);
        assert!(input.read_object().await.is_err());
    }
}

struct StreamMappedData(RdbcInputStream, RdbcReader);
#[async_trait::async_trait]
impl SqlData for StreamMappedData {
    async fn sql_type_name(&mut self) -> Result<String, RdbcError> {
        Ok("app.streams".into())
    }
    async fn read_sql(&mut self, input: &mut dyn SqlInput, _name: &str) -> Result<(), RdbcError> {
        self.0 = input
            .read_binary_stream()
            .await?
            .ok_or_else(|| RdbcError::InvalidArgument("null binary".into()))?;
        self.1 = input
            .read_character_stream()
            .await?
            .ok_or_else(|| RdbcError::InvalidArgument("null reader".into()))?;
        Ok(())
    }
    async fn write_sql(&mut self, output: &mut dyn SqlOutput) -> Result<(), RdbcError> {
        output.write_binary_stream(Some(self.0.clone())).await?;
        output.write_character_stream(Some(self.1.clone())).await
    }
}
#[tokio::test]
async fn nested_udt_streams_preserve_identity_and_are_not_materialized() {
    let bytes = RdbcInputStream::from_bytes(vec![0, 255, 128]);
    let chars = RdbcReader::from_utf16(vec![0xD800, 0x0061]);
    let mut data = StreamMappedData(bytes.clone(), chars.clone());
    let mut output = BufferedSqlOutput::new();
    output.write_object(Some(&mut data)).await.unwrap();
    let RdbcObject::Struct(value) = output.into_input().read_object().await.unwrap() else {
        panic!("missing structured attribute")
    };
    let mut input = BufferedSqlInput::new(value.attributes().await.unwrap());
    let read_bytes = input.read_binary_stream().await.unwrap().unwrap();
    let read_chars = input.read_character_stream().await.unwrap().unwrap();
    assert_eq!(read_bytes, bytes);
    assert_eq!(read_chars, chars);
    assert_eq!(bytes.read_to_end().unwrap(), vec![0, 255, 128]);
    assert_eq!(chars.read_to_end_utf16().unwrap(), vec![0xD800, 0x0061]);
    assert!(read_bytes.read_to_end().unwrap().is_empty());
    assert!(read_chars.read_to_end_utf16().unwrap().is_empty());
}

struct SendOnlySqlType(std::cell::Cell<i32>);
impl druid_rdbc::types::SqlType for SendOnlySqlType {
    fn name(&self) -> &'static str {
        "VENDOR_INTEGER"
    }
    fn vendor(&self) -> &'static str {
        "example"
    }
    fn vendor_type_number(&self) -> i32 {
        self.0.get()
    }
}
#[tokio::test]
async fn default_typed_output_does_not_add_sync_to_the_standard_sql_type_contract() {
    fn require_send<T: std::future::Future + Send>(future: T) -> T {
        future
    }
    let mut output = BufferedSqlOutput::new();
    let future = {
        let descriptor = SendOnlySqlType(std::cell::Cell::new(73));
        require_send(output.write_object_as(None, &descriptor))
    };
    let error = future.await.unwrap_err();
    assert!(
        matches!(error, RdbcError::SqlException(ref e) if e.class_name() == "java.sql.SQLFeatureNotSupportedException")
    );
    assert!(output.into_input().read_object().await.is_err());
}
