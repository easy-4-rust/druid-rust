//! 固定 Java RowSet 的完整 85 setter 重载与标准身份/属性合同。
use druid_rdbc::connection::TransactionIsolation;
use druid_rdbc::extensions::{RowSet, RowSetInternal, SharedRowSet};
use druid_rdbc::result_set::ResultSet;
use druid_rdbc::statement::{
    ParameterIndex, PreparedInputParameter, PreparedStatement, PreparedTypeNameArgument,
    ResultSetConcurrency, ResultSetType,
};
use druid_rdbc::types::{
    RdbcCalendarArgument, RdbcCharacterLength, RdbcObject, RdbcStreamLength, RdbcTypeMap,
};
use druid_rdbc::wrapper::{StandardUnwrapped, StandardUnwrappedMut, Wrapper};
use std::any::TypeId;
use std::sync::{Arc, Mutex};

#[test]
fn every_fixed_java_row_set_setter_preserves_its_family_and_overload() {
    let mut source = SharedRowSet::new();
    source
        .set_named_parameter_resolver(Some(Arc::new(|_, name| {
            assert_eq!(name, " 参数原名 ");
            ParameterIndex::new(1)
        })))
        .unwrap();
    // Java 固定声明：void setNull(int parameterIndex, int sqlType) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_null(ParameterIndex::new(1).unwrap(), 37)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Null {
            sql_type: 37,
            type_name: PreparedTypeNameArgument::Unspecified
        }],
        "set_null"
    );
    // Java 固定声明：void setNull(String parameterName, int sqlType) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_null_by_name(Some(" 参数原名 "), 37).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Null {
            sql_type: 37,
            type_name: PreparedTypeNameArgument::Unspecified
        }],
        "set_null_by_name"
    );
    // Java 固定声明：void setNull (int paramIndex, int sqlType, String typeName) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_null_with_type_name(ParameterIndex::new(1).unwrap(), 37, Some(" 类型或字符串 "))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Null {
            sql_type: 37,
            type_name: PreparedTypeNameArgument::Specified(Some(" 类型或字符串 ".into()))
        }],
        "set_null_with_type_name"
    );
    // Java 固定声明：void setNull (String parameterName, int sqlType, String typeName) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_null_with_type_name_by_name(Some(" 参数原名 "), 37, Some(" 类型或字符串 "))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Null {
            sql_type: 37,
            type_name: PreparedTypeNameArgument::Specified(Some(" 类型或字符串 ".into()))
        }],
        "set_null_with_type_name_by_name"
    );
    // Java 固定声明：void setBoolean(int parameterIndex, boolean x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_boolean(ParameterIndex::new(1).unwrap(), true)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Boolean(true)],
        "set_boolean"
    );
    // Java 固定声明：void setBoolean(String parameterName, boolean x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_boolean_by_name(Some(" 参数原名 "), true)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Boolean(true)],
        "set_boolean_by_name"
    );
    // Java 固定声明：void setByte(int parameterIndex, byte x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_byte(ParameterIndex::new(1).unwrap(), 37)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Byte(37)],
        "set_byte"
    );
    // Java 固定声明：void setByte(String parameterName, byte x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_byte_by_name(Some(" 参数原名 "), 37).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Byte(37)],
        "set_byte_by_name"
    );
    // Java 固定声明：void setShort(int parameterIndex, short x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_short(ParameterIndex::new(1).unwrap(), 37)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Short(37)],
        "set_short"
    );
    // Java 固定声明：void setShort(String parameterName, short x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_short_by_name(Some(" 参数原名 "), 37).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Short(37)],
        "set_short_by_name"
    );
    // Java 固定声明：void setInt(int parameterIndex, int x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_int(ParameterIndex::new(1).unwrap(), 37)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Int(37)],
        "set_int"
    );
    // Java 固定声明：void setInt(String parameterName, int x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_int_by_name(Some(" 参数原名 "), 37).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Int(37)],
        "set_int_by_name"
    );
    // Java 固定声明：void setLong(int parameterIndex, long x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_long(ParameterIndex::new(1).unwrap(), 37)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Long(37)],
        "set_long"
    );
    // Java 固定声明：void setLong(String parameterName, long x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_long_by_name(Some(" 参数原名 "), 37).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Long(37)],
        "set_long_by_name"
    );
    // Java 固定声明：void setFloat(int parameterIndex, float x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_float(ParameterIndex::new(1).unwrap(), 1.25)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Float(1.25)],
        "set_float"
    );
    // Java 固定声明：void setFloat(String parameterName, float x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_float_by_name(Some(" 参数原名 "), 1.25).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Float(1.25)],
        "set_float_by_name"
    );
    // Java 固定声明：void setDouble(int parameterIndex, double x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_double(ParameterIndex::new(1).unwrap(), 1.25)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Double(1.25)],
        "set_double"
    );
    // Java 固定声明：void setDouble(String parameterName, double x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_double_by_name(Some(" 参数原名 "), 1.25)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Double(1.25)],
        "set_double_by_name"
    );
    // Java 固定声明：void setBigDecimal(int parameterIndex, BigDecimal x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_big_decimal(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BigDecimal(None)],
        "set_big_decimal"
    );
    // Java 固定声明：void setBigDecimal(String parameterName, BigDecimal x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_big_decimal_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BigDecimal(None)],
        "set_big_decimal_by_name"
    );
    // Java 固定声明：void setString(int parameterIndex, String x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_string(ParameterIndex::new(1).unwrap(), Some(" 类型或字符串 "))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::String(Some(
            " 类型或字符串 ".into()
        ))],
        "set_string"
    );
    // Java 固定声明：void setString(String parameterName, String x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_string_by_name(Some(" 参数原名 "), Some(" 类型或字符串 "))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::String(Some(
            " 类型或字符串 ".into()
        ))],
        "set_string_by_name"
    );
    // Java 固定声明：void setBytes(int parameterIndex, byte x[]) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_bytes(ParameterIndex::new(1).unwrap(), Some(vec![0, 255, 3]))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Bytes(Some(vec![0, 255, 3]))],
        "set_bytes"
    );
    // Java 固定声明：void setBytes(String parameterName, byte x[]) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_bytes_by_name(Some(" 参数原名 "), Some(vec![0, 255, 3]))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Bytes(Some(vec![0, 255, 3]))],
        "set_bytes_by_name"
    );
    // Java 固定声明：void setDate(int parameterIndex, java.sql.Date x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_date(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Date {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified
        }],
        "set_date"
    );
    // Java 固定声明：void setTime(int parameterIndex, java.sql.Time x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_time(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Time {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified
        }],
        "set_time"
    );
    // Java 固定声明：void setTimestamp(int parameterIndex, java.sql.Timestamp x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_timestamp(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Timestamp {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified
        }],
        "set_timestamp"
    );
    // Java 固定声明：void setTimestamp(String parameterName, java.sql.Timestamp x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_timestamp_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Timestamp {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified
        }],
        "set_timestamp_by_name"
    );
    // Java 固定声明：void setAsciiStream(int parameterIndex, java.io.InputStream x, int length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_ascii_stream_with_length(ParameterIndex::new(1).unwrap(), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::AsciiStream {
            stream: None,
            length: RdbcStreamLength::Int(-3)
        }],
        "set_ascii_stream_with_length"
    );
    // Java 固定声明：void setAsciiStream(String parameterName, java.io.InputStream x, int length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_ascii_stream_with_length_by_name(Some(" 参数原名 "), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::AsciiStream {
            stream: None,
            length: RdbcStreamLength::Int(-3)
        }],
        "set_ascii_stream_with_length_by_name"
    );
    // Java 固定声明：void setBinaryStream(int parameterIndex, java.io.InputStream x, int length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_binary_stream_with_length(ParameterIndex::new(1).unwrap(), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BinaryStream {
            stream: None,
            length: RdbcStreamLength::Int(-3)
        }],
        "set_binary_stream_with_length"
    );
    // Java 固定声明：void setBinaryStream(String parameterName, java.io.InputStream x, int length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_binary_stream_with_length_by_name(Some(" 参数原名 "), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BinaryStream {
            stream: None,
            length: RdbcStreamLength::Int(-3)
        }],
        "set_binary_stream_with_length_by_name"
    );
    // Java 固定声明：void setCharacterStream(int parameterIndex, Reader reader, int length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_character_stream_with_length(ParameterIndex::new(1).unwrap(), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::CharacterStream {
            reader: None,
            length: RdbcCharacterLength::Int(-3)
        }],
        "set_character_stream_with_length"
    );
    // Java 固定声明：void setCharacterStream(String parameterName, java.io.Reader reader, int length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_character_stream_with_length_by_name(Some(" 参数原名 "), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::CharacterStream {
            reader: None,
            length: RdbcCharacterLength::Int(-3)
        }],
        "set_character_stream_with_length_by_name"
    );
    // Java 固定声明：void setAsciiStream(int parameterIndex, java.io.InputStream x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_ascii_stream(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::AsciiStream {
            stream: None,
            length: RdbcStreamLength::Unspecified
        }],
        "set_ascii_stream"
    );
    // Java 固定声明：void setAsciiStream(String parameterName, java.io.InputStream x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_ascii_stream_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::AsciiStream {
            stream: None,
            length: RdbcStreamLength::Unspecified
        }],
        "set_ascii_stream_by_name"
    );
    // Java 固定声明：void setBinaryStream(int parameterIndex, java.io.InputStream x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_binary_stream(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BinaryStream {
            stream: None,
            length: RdbcStreamLength::Unspecified
        }],
        "set_binary_stream"
    );
    // Java 固定声明：void setBinaryStream(String parameterName, java.io.InputStream x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_binary_stream_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BinaryStream {
            stream: None,
            length: RdbcStreamLength::Unspecified
        }],
        "set_binary_stream_by_name"
    );
    // Java 固定声明：void setCharacterStream(int parameterIndex, java.io.Reader reader) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_character_stream(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::CharacterStream {
            reader: None,
            length: RdbcCharacterLength::Unspecified
        }],
        "set_character_stream"
    );
    // Java 固定声明：void setCharacterStream(String parameterName, java.io.Reader reader) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_character_stream_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::CharacterStream {
            reader: None,
            length: RdbcCharacterLength::Unspecified
        }],
        "set_character_stream_by_name"
    );
    // Java 固定声明：void setNCharacterStream(int parameterIndex, Reader value) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_character_stream(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NCharacterStream {
            reader: None,
            length: RdbcCharacterLength::Unspecified
        }],
        "set_n_character_stream"
    );
    // Java 固定声明：void setObject(int parameterIndex, Object x, int targetSqlType, int scaleOrLength) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_object_with_type_and_scale_or_length(
                ParameterIndex::new(1).unwrap(),
                Some(RdbcObject::String("对象".into())),
                37,
                37,
            )
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Object {
            value: Some(RdbcObject::String("对象".into())),
            target_sql_type: Some(37),
            scale_or_length: Some(37)
        }],
        "set_object_with_type_and_scale_or_length"
    );
    // Java 固定声明：void setObject(String parameterName, Object x, int targetSqlType, int scale) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_object_with_type_and_scale_or_length_by_name(
                Some(" 参数原名 "),
                Some(RdbcObject::String("对象".into())),
                37,
                37,
            )
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Object {
            value: Some(RdbcObject::String("对象".into())),
            target_sql_type: Some(37),
            scale_or_length: Some(37)
        }],
        "set_object_with_type_and_scale_or_length_by_name"
    );
    // Java 固定声明：void setObject(int parameterIndex, Object x, int targetSqlType) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_object_with_type(
                ParameterIndex::new(1).unwrap(),
                Some(RdbcObject::String("对象".into())),
                37,
            )
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Object {
            value: Some(RdbcObject::String("对象".into())),
            target_sql_type: Some(37),
            scale_or_length: None
        }],
        "set_object_with_type"
    );
    // Java 固定声明：void setObject(String parameterName, Object x, int targetSqlType) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_object_with_type_by_name(
                Some(" 参数原名 "),
                Some(RdbcObject::String("对象".into())),
                37,
            )
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Object {
            value: Some(RdbcObject::String("对象".into())),
            target_sql_type: Some(37),
            scale_or_length: None
        }],
        "set_object_with_type_by_name"
    );
    // Java 固定声明：void setObject(String parameterName, Object x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_object_by_name(Some(" 参数原名 "), Some(RdbcObject::String("对象".into())))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Object {
            value: Some(RdbcObject::String("对象".into())),
            target_sql_type: None,
            scale_or_length: None
        }],
        "set_object_by_name"
    );
    // Java 固定声明：void setObject(int parameterIndex, Object x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_object(
                ParameterIndex::new(1).unwrap(),
                Some(RdbcObject::String("对象".into())),
            )
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Object {
            value: Some(RdbcObject::String("对象".into())),
            target_sql_type: None,
            scale_or_length: None
        }],
        "set_object"
    );
    // Java 固定声明：void setRef (int i, Ref x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_ref(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Ref(None)],
        "set_ref"
    );
    // Java 固定声明：void setBlob (int i, Blob x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_blob(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Blob(None)],
        "set_blob"
    );
    // Java 固定声明：void setBlob(int parameterIndex, InputStream inputStream, long length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_blob_stream_with_length(ParameterIndex::new(1).unwrap(), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BlobStream {
            stream: None,
            length: RdbcStreamLength::Long(-3)
        }],
        "set_blob_stream_with_length"
    );
    // Java 固定声明：void setBlob(int parameterIndex, InputStream inputStream) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_blob_stream(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BlobStream {
            stream: None,
            length: RdbcStreamLength::Unspecified
        }],
        "set_blob_stream"
    );
    // Java 固定声明：void setBlob(String parameterName, InputStream inputStream, long length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_blob_stream_with_length_by_name(Some(" 参数原名 "), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BlobStream {
            stream: None,
            length: RdbcStreamLength::Long(-3)
        }],
        "set_blob_stream_with_length_by_name"
    );
    // Java 固定声明：void setBlob (String parameterName, Blob x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_blob_by_name(Some(" 参数原名 "), None).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Blob(None)],
        "set_blob_by_name"
    );
    // Java 固定声明：void setBlob(String parameterName, InputStream inputStream) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_blob_stream_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::BlobStream {
            stream: None,
            length: RdbcStreamLength::Unspecified
        }],
        "set_blob_stream_by_name"
    );
    // Java 固定声明：void setClob (int i, Clob x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_clob(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Clob(None)],
        "set_clob"
    );
    // Java 固定声明：void setClob(int parameterIndex, Reader reader, long length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_clob_reader_with_length(ParameterIndex::new(1).unwrap(), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::ClobReader {
            reader: None,
            length: RdbcCharacterLength::Long(-3)
        }],
        "set_clob_reader_with_length"
    );
    // Java 固定声明：void setClob(int parameterIndex, Reader reader) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_clob_reader(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::ClobReader {
            reader: None,
            length: RdbcCharacterLength::Unspecified
        }],
        "set_clob_reader"
    );
    // Java 固定声明：void setClob(String parameterName, Reader reader, long length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_clob_reader_with_length_by_name(Some(" 参数原名 "), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::ClobReader {
            reader: None,
            length: RdbcCharacterLength::Long(-3)
        }],
        "set_clob_reader_with_length_by_name"
    );
    // Java 固定声明：void setClob (String parameterName, Clob x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_clob_by_name(Some(" 参数原名 "), None).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Clob(None)],
        "set_clob_by_name"
    );
    // Java 固定声明：void setClob(String parameterName, Reader reader) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_clob_reader_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::ClobReader {
            reader: None,
            length: RdbcCharacterLength::Unspecified
        }],
        "set_clob_reader_by_name"
    );
    // Java 固定声明：void setArray (int i, Array x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_array(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Array(None)],
        "set_array"
    );
    // Java 固定声明：void setDate(int parameterIndex, java.sql.Date x, Calendar cal) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_date_with_calendar(ParameterIndex::new(1).unwrap(), None, None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Date {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None)
        }],
        "set_date_with_calendar"
    );
    // Java 固定声明：void setDate(String parameterName, java.sql.Date x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_date_by_name(Some(" 参数原名 "), None).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Date {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified
        }],
        "set_date_by_name"
    );
    // Java 固定声明：void setDate(String parameterName, java.sql.Date x, Calendar cal) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_date_with_calendar_by_name(Some(" 参数原名 "), None, None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Date {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None)
        }],
        "set_date_with_calendar_by_name"
    );
    // Java 固定声明：void setTime(int parameterIndex, java.sql.Time x, Calendar cal) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_time_with_calendar(ParameterIndex::new(1).unwrap(), None, None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Time {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None)
        }],
        "set_time_with_calendar"
    );
    // Java 固定声明：void setTime(String parameterName, java.sql.Time x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set.set_time_by_name(Some(" 参数原名 "), None).unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Time {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified
        }],
        "set_time_by_name"
    );
    // Java 固定声明：void setTime(String parameterName, java.sql.Time x, Calendar cal) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_time_with_calendar_by_name(Some(" 参数原名 "), None, None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Time {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None)
        }],
        "set_time_with_calendar_by_name"
    );
    // Java 固定声明：void setTimestamp(int parameterIndex, java.sql.Timestamp x, Calendar cal) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_timestamp_with_calendar(ParameterIndex::new(1).unwrap(), None, None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Timestamp {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None)
        }],
        "set_timestamp_with_calendar"
    );
    // Java 固定声明：void setTimestamp(String parameterName, java.sql.Timestamp x, Calendar cal) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_timestamp_with_calendar_by_name(Some(" 参数原名 "), None, None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Timestamp {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None)
        }],
        "set_timestamp_with_calendar_by_name"
    );
    // Java 固定声明：void setSQLXML(int parameterIndex, SQLXML xmlObject) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_sql_xml(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::SqlXml(None)],
        "set_sql_xml"
    );
    // Java 固定声明：void setSQLXML(String parameterName, SQLXML xmlObject) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_sql_xml_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::SqlXml(None)],
        "set_sql_xml_by_name"
    );
    // Java 固定声明：void setRowId(int parameterIndex, RowId x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_row_id(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::RowId(None)],
        "set_row_id"
    );
    // Java 固定声明：void setRowId(String parameterName, RowId x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_row_id_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::RowId(None)],
        "set_row_id_by_name"
    );
    // Java 固定声明：void setNString(int parameterIndex, String value) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_string(ParameterIndex::new(1).unwrap(), Some(" 类型或字符串 "))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NString(Some(
            " 类型或字符串 ".into()
        ))],
        "set_n_string"
    );
    // Java 固定声明：public void setNString(String parameterName, String value) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_string_by_name(Some(" 参数原名 "), Some(" 类型或字符串 "))
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NString(Some(
            " 类型或字符串 ".into()
        ))],
        "set_n_string_by_name"
    );
    // Java 固定声明：void setNCharacterStream(int parameterIndex, Reader value, long length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_character_stream_with_length(ParameterIndex::new(1).unwrap(), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NCharacterStream {
            reader: None,
            length: RdbcCharacterLength::Long(-3)
        }],
        "set_n_character_stream_with_length"
    );
    // Java 固定声明：public void setNCharacterStream(String parameterName, Reader value, long length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_character_stream_with_length_by_name(Some(" 参数原名 "), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NCharacterStream {
            reader: None,
            length: RdbcCharacterLength::Long(-3)
        }],
        "set_n_character_stream_with_length_by_name"
    );
    // Java 固定声明：void setNCharacterStream(String parameterName, Reader value) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_character_stream_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NCharacterStream {
            reader: None,
            length: RdbcCharacterLength::Unspecified
        }],
        "set_n_character_stream_by_name"
    );
    // Java 固定声明：void setNClob(String parameterName, NClob value) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_clob_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NClob(None)],
        "set_n_clob_by_name"
    );
    // Java 固定声明：void setNClob(String parameterName, Reader reader, long length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_clob_reader_with_length_by_name(Some(" 参数原名 "), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NClobReader {
            reader: None,
            length: RdbcCharacterLength::Long(-3)
        }],
        "set_n_clob_reader_with_length_by_name"
    );
    // Java 固定声明：void setNClob(String parameterName, Reader reader) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_clob_reader_by_name(Some(" 参数原名 "), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NClobReader {
            reader: None,
            length: RdbcCharacterLength::Unspecified
        }],
        "set_n_clob_reader_by_name"
    );
    // Java 固定声明：void setNClob(int parameterIndex, Reader reader, long length) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_clob_reader_with_length(ParameterIndex::new(1).unwrap(), None, -3)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NClobReader {
            reader: None,
            length: RdbcCharacterLength::Long(-3)
        }],
        "set_n_clob_reader_with_length"
    );
    // Java 固定声明：void setNClob(int parameterIndex, NClob value) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_clob(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NClob(None)],
        "set_n_clob"
    );
    // Java 固定声明：void setNClob(int parameterIndex, Reader reader) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_n_clob_reader(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::NClobReader {
            reader: None,
            length: RdbcCharacterLength::Unspecified
        }],
        "set_n_clob_reader"
    );
    // Java 固定声明：void setURL(int parameterIndex, java.net.URL x) throws SQLException;
    source.clear_parameters().unwrap();
    {
        let row_set: &mut dyn RowSet = &mut source;
        row_set
            .set_parameter_url(ParameterIndex::new(1).unwrap(), None)
            .unwrap();
    }
    assert_eq!(
        source.params().unwrap(),
        vec![PreparedInputParameter::Url(None)],
        "set_parameter_url"
    );
}

#[test]
fn canonical_row_set_wrapper_preserves_leaf_parent_and_mutable_identity() {
    let mut source = SharedRowSet::new();
    for target in [
        TypeId::of::<dyn RowSet>(),
        TypeId::of::<dyn ResultSet>(),
        TypeId::of::<SharedRowSet>(),
        TypeId::of::<dyn Wrapper>(),
    ] {
        assert!(source.is_wrapper_for(Some(target)).unwrap());
    }
    assert!(!source
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    let StandardUnwrapped::RowSet(leaf) = source.unwrap(Some(TypeId::of::<dyn RowSet>())).unwrap()
    else {
        panic!("RowSet leaf required")
    };
    assert!(std::ptr::addr_eq(leaf, &source as &dyn RowSet));
    let StandardUnwrapped::ResultSet(parent) =
        source.unwrap(Some(TypeId::of::<dyn ResultSet>())).unwrap()
    else {
        panic!("ResultSet parent required")
    };
    assert!(std::ptr::addr_eq(parent, &source as &dyn ResultSet));
    let StandardUnwrappedMut::RowSet(leaf) = source
        .unwrap_mut(Some(TypeId::of::<SharedRowSet>()))
        .unwrap()
    else {
        panic!("same mutable leaf required")
    };
    leaf.set_command(Some("通过同一可变视图")).unwrap();
    assert_eq!(source.command(), Some("通过同一可变视图".into()));
}

#[test]
fn canonical_row_set_properties_forward_all_requests_and_validate_atomically() {
    let mut source = SharedRowSet::new();
    let map = Arc::new(Mutex::new(RdbcTypeMap::new()));
    let row_set: &mut dyn RowSet = &mut source;
    row_set.set_url(Some(" jdbc:exact ")).unwrap();
    row_set.set_data_source_name(Some(" 名称 ")).unwrap();
    row_set.set_username(Some("用户")).unwrap();
    row_set.set_password(Some("不能出现在Debug的密码")).unwrap();
    row_set.set_command(Some(" select ? ")).unwrap();
    row_set.set_transaction_isolation(8).unwrap();
    row_set.set_type_map(Arc::clone(&map)).unwrap();
    row_set.set_read_only(false).unwrap();
    row_set.set_max_field_size(17).unwrap();
    row_set.set_max_rows(19).unwrap();
    row_set.set_escape_processing(false).unwrap();
    row_set.set_query_timeout(23).unwrap();
    row_set.set_type(1004).unwrap();
    row_set.set_concurrency(1008).unwrap();
    assert_eq!(row_set.url(), Some(" jdbc:exact ".into()));
    assert_eq!(row_set.data_source_name(), Some(" 名称 ".into()));
    assert_eq!(row_set.username(), Some("用户".into()));
    assert_eq!(row_set.password(), Some("不能出现在Debug的密码".into()));
    assert_eq!(row_set.command(), Some(" select ? ".into()));
    assert_eq!(
        row_set.transaction_isolation(),
        TransactionIsolation::SERIALIZABLE
    );
    assert!(Arc::ptr_eq(&row_set.type_map(), &map));
    assert!(!row_set.is_read_only());
    assert_eq!(row_set.max_field_size(), 17);
    assert_eq!(row_set.max_rows(), 19);
    assert!(!row_set.escape_processing());
    assert_eq!(row_set.query_timeout(), 23);
    assert_eq!(
        row_set.result_set_type().unwrap(),
        ResultSetType::ScrollInsensitive
    );
    assert_eq!(
        row_set.concurrency().unwrap(),
        ResultSetConcurrency::Updatable
    );
    assert!(row_set.set_max_rows(-1).is_err());
    assert!(row_set.set_max_field_size(-1).is_err());
    assert!(row_set.set_query_timeout(-1).is_err());
    assert!(row_set.set_transaction_isolation(0).is_err());
    assert!(row_set.set_type(1002).is_err());
    assert!(row_set.set_concurrency(1006).is_err());
    assert_eq!(row_set.max_rows(), 19);
    assert_eq!(
        row_set.transaction_isolation(),
        TransactionIsolation::SERIALIZABLE
    );
    assert_eq!(
        row_set.result_set_type().unwrap(),
        ResultSetType::ScrollInsensitive
    );
    assert_eq!(
        row_set.concurrency().unwrap(),
        ResultSetConcurrency::Updatable
    );
    assert!(!format!("{source:?}").contains("不能出现在Debug的密码"));
}
