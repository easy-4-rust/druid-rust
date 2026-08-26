use crate::error::RdbcError;
use crate::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use crate::types::{
    RdbcArray, RdbcBigDecimal as BigDecimal, RdbcBlob, RdbcCalendarArgument, RdbcCharacterLength,
    RdbcClob, RdbcInputStream, RdbcNClob, RdbcNaiveDate as NaiveDate,
    RdbcNaiveDateTime as NaiveDateTime, RdbcNaiveTime as NaiveTime, RdbcObject, RdbcReader,
    RdbcRef, RdbcRowId, RdbcSqlXml, RdbcStreamLength, RdbcTargetType, RdbcTypeMap, RdbcUrl, Value,
};

/// `ResultSet` 的 JDBC-like typed getter/update convenience surface。
///
/// 本 trait 不拥有 cursor state；所有实现均委托唯一 [`ResultSet`] core，因此不会形成
/// 第二套 raw/logical hierarchy。
pub trait ResultSetExt: ResultSet {
    /// 读取可空 String。
    fn string(&self, column: ColumnIndex) -> Result<Option<String>, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(None),
            Value::String(value) => Ok(Some(value)),
            other => Err(result_set_type_error("String", &RdbcObject::Scalar(other))),
        }
    }

    /// 按 label 读取可空 String。
    fn string_by_label(&self, label: &str) -> Result<Option<String>, RdbcError> {
        self.string(self.find_column(label)?)
    }

    /// 读取可空 national String。
    fn n_string(&self, column: ColumnIndex) -> Result<Option<String>, RdbcError> {
        match self.object(column)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::NString(value) | RdbcObject::Scalar(Value::String(value)) => {
                Ok(Some(value))
            }
            other => Err(result_set_type_error("NString", &other)),
        }
    }

    /// 按 label 读取可空 national String。
    fn n_string_by_label(&self, label: &str) -> Result<Option<String>, RdbcError> {
        self.n_string(self.find_column(label)?)
    }

    /// 读取 boolean；SQL NULL 返回 false。
    fn boolean(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(false),
            Value::Bool(value) => Ok(value),
            other => Err(result_set_type_error("boolean", &RdbcObject::Scalar(other))),
        }
    }

    /// 按 label 读取 boolean。
    fn boolean_by_label(&self, label: &str) -> Result<bool, RdbcError> {
        self.boolean(self.find_column(label)?)
    }

    /// 读取 long；SQL NULL 返回 0。
    fn long(&self, column: ColumnIndex) -> Result<i64, RdbcError> {
        integer_value(self.value(column)?, "long")
    }

    /// 按 label 读取 long。
    fn long_by_label(&self, label: &str) -> Result<i64, RdbcError> {
        self.long(self.find_column(label)?)
    }

    /// 读取 int，并拒绝超出 i32 的值。
    fn int(&self, column: ColumnIndex) -> Result<i32, RdbcError> {
        i32::try_from(self.long(column)?)
            .map_err(|_| RdbcError::DriverError("ResultSet value exceeds int".to_owned()))
    }

    /// 按 label 读取 int。
    fn int_by_label(&self, label: &str) -> Result<i32, RdbcError> {
        self.int(self.find_column(label)?)
    }

    /// 读取 short，并拒绝超出 i16 的值。
    fn short(&self, column: ColumnIndex) -> Result<i16, RdbcError> {
        i16::try_from(self.long(column)?)
            .map_err(|_| RdbcError::DriverError("ResultSet value exceeds short".to_owned()))
    }

    /// 按 label 读取 short。
    fn short_by_label(&self, label: &str) -> Result<i16, RdbcError> {
        self.short(self.find_column(label)?)
    }

    /// 读取 byte，并拒绝超出 i8 的值。
    fn byte(&self, column: ColumnIndex) -> Result<i8, RdbcError> {
        i8::try_from(self.long(column)?)
            .map_err(|_| RdbcError::DriverError("ResultSet value exceeds byte".to_owned()))
    }

    /// 按 label 读取 byte。
    fn byte_by_label(&self, label: &str) -> Result<i8, RdbcError> {
        self.byte(self.find_column(label)?)
    }

    /// 读取 double；SQL NULL 返回 0.0。
    fn double(&self, column: ColumnIndex) -> Result<f64, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(0.0),
            Value::Float(value) => Ok(value),
            other => Err(result_set_type_error("double", &RdbcObject::Scalar(other))),
        }
    }

    /// 按 label 读取 double。
    fn double_by_label(&self, label: &str) -> Result<f64, RdbcError> {
        self.double(self.find_column(label)?)
    }

    /// 读取 float，保持 Java narrowing conversion。
    #[allow(clippy::cast_possible_truncation)]
    fn float(&self, column: ColumnIndex) -> Result<f32, RdbcError> {
        Ok(self.double(column)? as f32)
    }

    /// 按 label 读取 float。
    fn float_by_label(&self, label: &str) -> Result<f32, RdbcError> {
        self.float(self.find_column(label)?)
    }

    /// 读取可空 bytes。
    fn bytes(&self, column: ColumnIndex) -> Result<Option<Vec<u8>>, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(None),
            Value::Bytes(value) => Ok(Some(value)),
            other => Err(result_set_type_error("bytes", &RdbcObject::Scalar(other))),
        }
    }

    /// 按 label 读取可空 bytes。
    fn bytes_by_label(&self, label: &str) -> Result<Option<Vec<u8>>, RdbcError> {
        self.bytes(self.find_column(label)?)
    }

    /// 读取可空 decimal。
    fn big_decimal(&self, column: ColumnIndex) -> Result<Option<BigDecimal>, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(None),
            Value::Decimal(value) => Ok(Some(value)),
            Value::Int(value) => Ok(Some(BigDecimal::from(value))),
            Value::Float(value) => value
                .to_string()
                .parse::<BigDecimal>()
                .map(Some)
                .map_err(|error| {
                    RdbcError::DriverError(format!(
                        "ResultSet floating point value {value} cannot be represented as BigDecimal: {error}"
                    ))
                }),
            Value::String(value) => value
                .parse::<BigDecimal>()
                .map(Some)
                .map_err(|error| {
                    RdbcError::DriverError(format!(
                        "ResultSet value {value:?} cannot be converted to BigDecimal: {error}"
                    ))
                }),
            other => Err(result_set_type_error(
                "BigDecimal",
                &RdbcObject::Scalar(other),
            )),
        }
    }

    /// 按 label 读取可空 decimal。
    fn big_decimal_by_label(&self, label: &str) -> Result<Option<BigDecimal>, RdbcError> {
        self.big_decimal(self.find_column(label)?)
    }

    /// 读取可空 Date，并保留 Calendar argument identity。
    fn date(
        &self,
        column: ColumnIndex,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(None),
            Value::Date(value) => Ok(Some(value)),
            other => Err(result_set_type_error("Date", &RdbcObject::Scalar(other))),
        }
    }

    /// 按 label 读取可空 Date。
    fn date_by_label(
        &self,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, RdbcError> {
        self.date(self.find_column(label)?, calendar)
    }

    /// 读取可空 Time，并保留 Calendar argument identity。
    fn time(
        &self,
        column: ColumnIndex,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(None),
            Value::Time(value) => Ok(Some(value)),
            other => Err(result_set_type_error("Time", &RdbcObject::Scalar(other))),
        }
    }

    /// 按 label 读取可空 Time。
    fn time_by_label(
        &self,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, RdbcError> {
        self.time(self.find_column(label)?, calendar)
    }

    /// 读取可空 Timestamp，并保留 Calendar argument identity。
    fn timestamp(
        &self,
        column: ColumnIndex,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, RdbcError> {
        match self.value(column)? {
            Value::Null => Ok(None),
            Value::Timestamp(value) => Ok(Some(value)),
            other => Err(result_set_type_error(
                "Timestamp",
                &RdbcObject::Scalar(other),
            )),
        }
    }

    /// 按 label 读取可空 Timestamp。
    fn timestamp_by_label(
        &self,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, RdbcError> {
        self.timestamp(self.find_column(label)?, calendar)
    }

    /// 使用 type map 按 label 读取 object。
    fn object_by_label_with_type_map(
        &self,
        label: &str,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.object_with_type_map(self.find_column(label)?, type_map)
    }

    /// 使用 target type 按 label 读取 object。
    fn object_by_label_as(
        &self,
        label: &str,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.object_as(self.find_column(label)?, target_type)
    }

    /// 读取 Ref handle。
    fn reference(&self, column: ColumnIndex) -> Result<Option<RdbcRef>, RdbcError> {
        object_variant(self.object(column)?, "Ref", |value| match value {
            RdbcObject::Ref(value) => Some(value),
            _ => None,
        })
    }

    /// 按 label 读取 Ref handle。
    fn reference_by_label(&self, label: &str) -> Result<Option<RdbcRef>, RdbcError> {
        self.reference(self.find_column(label)?)
    }

    /// 读取 Blob handle。
    fn blob(&self, column: ColumnIndex) -> Result<Option<RdbcBlob>, RdbcError> {
        object_variant(self.object(column)?, "Blob", |value| match value {
            RdbcObject::Blob(value) => Some(value),
            _ => None,
        })
    }

    /// 按 label 读取 Blob handle。
    fn blob_by_label(&self, label: &str) -> Result<Option<RdbcBlob>, RdbcError> {
        self.blob(self.find_column(label)?)
    }

    /// 读取 Clob handle。
    fn clob(&self, column: ColumnIndex) -> Result<Option<RdbcClob>, RdbcError> {
        object_variant(self.object(column)?, "Clob", |value| match value {
            RdbcObject::Clob(value) => Some(value),
            _ => None,
        })
    }

    /// 按 label 读取 Clob handle。
    fn clob_by_label(&self, label: &str) -> Result<Option<RdbcClob>, RdbcError> {
        self.clob(self.find_column(label)?)
    }

    /// 读取 Array handle。
    fn array(&self, column: ColumnIndex) -> Result<Option<RdbcArray>, RdbcError> {
        object_variant(self.object(column)?, "Array", |value| match value {
            RdbcObject::Array(value) => Some(value),
            _ => None,
        })
    }

    /// 按 label 读取 Array handle。
    fn array_by_label(&self, label: &str) -> Result<Option<RdbcArray>, RdbcError> {
        self.array(self.find_column(label)?)
    }

    /// 读取 URL value。
    fn url(&self, column: ColumnIndex) -> Result<Option<RdbcUrl>, RdbcError> {
        object_variant(self.object(column)?, "URL", |value| match value {
            RdbcObject::Url(value) => Some(value),
            _ => None,
        })
    }

    /// 按 label 读取 URL value。
    fn url_by_label(&self, label: &str) -> Result<Option<RdbcUrl>, RdbcError> {
        self.url(self.find_column(label)?)
    }

    /// 读取 RowId value。
    fn row_id(&self, column: ColumnIndex) -> Result<Option<RdbcRowId>, RdbcError> {
        object_variant(self.object(column)?, "RowId", |value| match value {
            RdbcObject::RowId(value) => Some(value),
            _ => None,
        })
    }

    /// 按 label 读取 RowId value。
    fn row_id_by_label(&self, label: &str) -> Result<Option<RdbcRowId>, RdbcError> {
        self.row_id(self.find_column(label)?)
    }

    /// 读取 NClob handle。
    fn n_clob(&self, column: ColumnIndex) -> Result<Option<RdbcNClob>, RdbcError> {
        object_variant(self.object(column)?, "NClob", |value| match value {
            RdbcObject::NClob(value) => Some(value),
            _ => None,
        })
    }

    /// 按 label 读取 NClob handle。
    fn n_clob_by_label(&self, label: &str) -> Result<Option<RdbcNClob>, RdbcError> {
        self.n_clob(self.find_column(label)?)
    }

    /// 读取 SQLXML handle。
    fn sql_xml(&self, column: ColumnIndex) -> Result<Option<RdbcSqlXml>, RdbcError> {
        object_variant(self.object(column)?, "SQLXML", |value| match value {
            RdbcObject::SqlXml(value) => Some(value),
            _ => None,
        })
    }

    /// 按 label 读取 SQLXML handle。
    fn sql_xml_by_label(&self, label: &str) -> Result<Option<RdbcSqlXml>, RdbcError> {
        self.sql_xml(self.find_column(label)?)
    }

    /// 读取 ASCII stream。
    fn ascii_stream(&self, column: ColumnIndex) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream(column, crate::result_set::ResultSetStreamKind::Ascii)
    }

    /// 按 label 读取 ASCII stream。
    fn ascii_stream_by_label(&self, label: &str) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.ascii_stream(self.find_column(label)?)
    }

    /// 读取 legacy Unicode stream。
    fn unicode_stream(&self, column: ColumnIndex) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream(column, crate::result_set::ResultSetStreamKind::Unicode)
    }

    /// 按 label 读取 legacy Unicode stream。
    fn unicode_stream_by_label(&self, label: &str) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.unicode_stream(self.find_column(label)?)
    }

    /// 读取 binary stream。
    fn binary_stream(&self, column: ColumnIndex) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream(column, crate::result_set::ResultSetStreamKind::Binary)
    }

    /// 按 label 读取 binary stream。
    fn binary_stream_by_label(&self, label: &str) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.binary_stream(self.find_column(label)?)
    }

    /// 读取 character Reader。
    fn character_stream(&self, column: ColumnIndex) -> Result<Option<RdbcReader>, RdbcError> {
        self.reader(column, false)
    }

    /// 按 label 读取 character Reader。
    fn character_stream_by_label(&self, label: &str) -> Result<Option<RdbcReader>, RdbcError> {
        self.character_stream(self.find_column(label)?)
    }

    /// 读取 national-character Reader。
    fn n_character_stream(&self, column: ColumnIndex) -> Result<Option<RdbcReader>, RdbcError> {
        self.reader(column, true)
    }

    /// 按 label 读取 national-character Reader。
    fn n_character_stream_by_label(&self, label: &str) -> Result<Option<RdbcReader>, RdbcError> {
        self.n_character_stream(self.find_column(label)?)
    }

    /// 更新 Ref handle。
    fn update_reference(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcRef>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, object_update(value.map(RdbcObject::Ref)))
    }

    /// 按 label 更新 Ref handle。
    fn update_reference_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcRef>,
    ) -> Result<(), RdbcError> {
        self.update_reference(self.find_column(label)?, value)
    }

    /// 更新 Blob handle。
    fn update_blob(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcBlob>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, object_update(value.map(RdbcObject::Blob)))
    }

    /// 按 label 更新 Blob handle。
    fn update_blob_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcBlob>,
    ) -> Result<(), RdbcError> {
        self.update_blob(self.find_column(label)?, value)
    }

    /// 更新 Clob handle。
    fn update_clob(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcClob>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, object_update(value.map(RdbcObject::Clob)))
    }

    /// 按 label 更新 Clob handle。
    fn update_clob_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcClob>,
    ) -> Result<(), RdbcError> {
        self.update_clob(self.find_column(label)?, value)
    }

    /// 更新 Array handle。
    fn update_array(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcArray>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, object_update(value.map(RdbcObject::Array)))
    }

    /// 按 label 更新 Array handle。
    fn update_array_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcArray>,
    ) -> Result<(), RdbcError> {
        self.update_array(self.find_column(label)?, value)
    }

    /// 更新 RowId value。
    fn update_row_id(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcRowId>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, object_update(value.map(RdbcObject::RowId)))
    }

    /// 按 label 更新 RowId value。
    fn update_row_id_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcRowId>,
    ) -> Result<(), RdbcError> {
        self.update_row_id(self.find_column(label)?, value)
    }

    /// 更新 NClob handle。
    fn update_n_clob(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcNClob>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, object_update(value.map(RdbcObject::NClob)))
    }

    /// 按 label 更新 NClob handle。
    fn update_n_clob_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcNClob>,
    ) -> Result<(), RdbcError> {
        self.update_n_clob(self.find_column(label)?, value)
    }

    /// 更新 SQLXML handle。
    fn update_sql_xml(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcSqlXml>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, object_update(value.map(RdbcObject::SqlXml)))
    }

    /// 按 label 更新 SQLXML handle。
    fn update_sql_xml_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcSqlXml>,
    ) -> Result<(), RdbcError> {
        self.update_sql_xml(self.find_column(label)?, value)
    }

    /// 使用 binary stream 更新 Blob column。
    fn update_blob_stream(
        &mut self,
        column: ColumnIndex,
        stream: Option<&RdbcInputStream>,
        length: RdbcStreamLength,
    ) -> Result<(), RdbcError> {
        self.update_value(
            column,
            ResultSetUpdate::BinaryStream {
                stream: stream.cloned(),
                length,
            },
        )
    }

    /// 按 label 使用 binary stream 更新 Blob column。
    fn update_blob_stream_by_label(
        &mut self,
        label: &str,
        stream: Option<&RdbcInputStream>,
        length: RdbcStreamLength,
    ) -> Result<(), RdbcError> {
        self.update_blob_stream(self.find_column(label)?, stream, length)
    }

    /// 使用 Reader 更新 Clob column。
    fn update_clob_reader(
        &mut self,
        column: ColumnIndex,
        reader: Option<&RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), RdbcError> {
        self.update_value(
            column,
            ResultSetUpdate::CharacterStream {
                reader: reader.cloned(),
                length,
            },
        )
    }

    /// 按 label 使用 Reader 更新 Clob column。
    fn update_clob_reader_by_label(
        &mut self,
        label: &str,
        reader: Option<&RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), RdbcError> {
        self.update_clob_reader(self.find_column(label)?, reader, length)
    }

    /// 使用 national Reader 更新 NClob column。
    fn update_n_clob_reader(
        &mut self,
        column: ColumnIndex,
        reader: Option<&RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), RdbcError> {
        self.update_value(
            column,
            ResultSetUpdate::NCharacterStream {
                reader: reader.cloned(),
                length,
            },
        )
    }

    /// 按 label 使用 national Reader 更新 NClob column。
    fn update_n_clob_reader_by_label(
        &mut self,
        label: &str,
        reader: Option<&RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), RdbcError> {
        self.update_n_clob_reader(self.find_column(label)?, reader, length)
    }
}

impl<T: ResultSet + ?Sized> ResultSetExt for T {}

fn integer_value(value: Value, expected: &str) -> Result<i64, RdbcError> {
    match value {
        Value::Null => Ok(0),
        Value::Int(value) => Ok(value),
        other => Err(result_set_type_error(expected, &RdbcObject::Scalar(other))),
    }
}

fn object_variant<T>(
    object: RdbcObject,
    expected: &str,
    convert: impl FnOnce(RdbcObject) -> Option<T>,
) -> Result<Option<T>, RdbcError> {
    if object.is_null() {
        return Ok(None);
    }
    convert(object.clone())
        .map(Some)
        .ok_or_else(|| result_set_type_error(expected, &object))
}

fn object_update(value: Option<RdbcObject>) -> ResultSetUpdate {
    value.map_or(ResultSetUpdate::Null, ResultSetUpdate::Object)
}

fn result_set_type_error(expected: &str, actual: &RdbcObject) -> RdbcError {
    RdbcError::DriverError(format!(
        "ResultSet expected {expected} column value, got {actual}"
    ))
}
