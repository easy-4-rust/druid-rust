use crate::error::RdbcError;
use crate::result_set::{ColumnIndex, ResultSet, ResultSetRead, ResultSetUpdate};
use crate::types::{decimal_from_f64, decimal_to_f64, decimal_to_i64};
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
        string_object(self.read_object(column, &ResultSetRead::String)?)
    }

    /// 按 label 读取可空 String。
    fn string_by_label(&self, label: &str) -> Result<Option<String>, RdbcError> {
        string_object(self.read_object_by_label(label, &ResultSetRead::String)?)
    }

    /// 按列读取可空 national String；返回 Unicode、SQL NULL 或原驱动错误。
    /// 对应 Java: java.sql.ResultSet#getNString(int)。
    fn n_string(&self, column: ColumnIndex) -> Result<Option<String>, RdbcError> {
        n_string_object(self.read_object(column, &ResultSetRead::NString)?)
    }

    /// 按原列名读取可空 national String；返回 Unicode、SQL NULL 或原驱动错误。
    /// 对应 Java: java.sql.ResultSet#getNString(String)。
    fn n_string_by_label(&self, label: &str) -> Result<Option<String>, RdbcError> {
        n_string_object(self.read_object_by_label(label, &ResultSetRead::NString)?)
    }

    /// 按列下标读取 boolean；SQL NULL 返回 false，保留数值及字符串转换规则。
    /// 对应 Java: java.sql.ResultSet#getBoolean(int)。返回转换后的布尔值或驱动错误。
    fn boolean(&self, column: ColumnIndex) -> Result<bool, RdbcError> {
        boolean_object(self.read_object(column, &ResultSetRead::Boolean)?)
    }

    /// 按 label 读取 boolean。
    fn boolean_by_label(&self, label: &str) -> Result<bool, RdbcError> {
        boolean_object(self.read_object_by_label(label, &ResultSetRead::Boolean)?)
    }

    /// 读取 long；SQL NULL 返回 0。
    fn long(&self, column: ColumnIndex) -> Result<i64, RdbcError> {
        long_object(self.read_object(column, &ResultSetRead::Long)?)
    }

    /// 按 label 读取 long。
    fn long_by_label(&self, label: &str) -> Result<i64, RdbcError> {
        long_object(self.read_object_by_label(label, &ResultSetRead::Long)?)
    }

    /// 读取 int，并拒绝超出 i32 的值。
    fn int(&self, column: ColumnIndex) -> Result<i32, RdbcError> {
        int_object(self.read_object(column, &ResultSetRead::Int)?)
    }

    /// 按 label 读取 int。
    fn int_by_label(&self, label: &str) -> Result<i32, RdbcError> {
        int_object(self.read_object_by_label(label, &ResultSetRead::Int)?)
    }

    /// 读取 short，并拒绝超出 i16 的值。
    fn short(&self, column: ColumnIndex) -> Result<i16, RdbcError> {
        short_object(self.read_object(column, &ResultSetRead::Short)?)
    }

    /// 按 label 读取 short。
    fn short_by_label(&self, label: &str) -> Result<i16, RdbcError> {
        short_object(self.read_object_by_label(label, &ResultSetRead::Short)?)
    }

    /// 读取 byte，并拒绝超出 i8 的值。
    fn byte(&self, column: ColumnIndex) -> Result<i8, RdbcError> {
        byte_object(self.read_object(column, &ResultSetRead::Byte)?)
    }

    /// 按 label 读取 byte。
    fn byte_by_label(&self, label: &str) -> Result<i8, RdbcError> {
        byte_object(self.read_object_by_label(label, &ResultSetRead::Byte)?)
    }

    /// 读取 double；SQL NULL 返回 0.0。
    fn double(&self, column: ColumnIndex) -> Result<f64, RdbcError> {
        double_object(self.read_object(column, &ResultSetRead::Double)?)
    }

    /// 按 label 读取 double。
    fn double_by_label(&self, label: &str) -> Result<f64, RdbcError> {
        double_object(self.read_object_by_label(label, &ResultSetRead::Double)?)
    }

    /// 读取 float，保持 Java narrowing conversion。
    #[allow(clippy::cast_possible_truncation)]
    fn float(&self, column: ColumnIndex) -> Result<f32, RdbcError> {
        float_object(self.read_object(column, &ResultSetRead::Float)?)
    }

    /// 按 label 读取 float。
    fn float_by_label(&self, label: &str) -> Result<f32, RdbcError> {
        float_object(self.read_object_by_label(label, &ResultSetRead::Float)?)
    }

    /// 读取可空 bytes。
    fn bytes(&self, column: ColumnIndex) -> Result<Option<Vec<u8>>, RdbcError> {
        bytes_object(self.read_object(column, &ResultSetRead::Bytes)?)
    }

    /// 按 label 读取可空 bytes。
    fn bytes_by_label(&self, label: &str) -> Result<Option<Vec<u8>>, RdbcError> {
        bytes_object(self.read_object_by_label(label, &ResultSetRead::Bytes)?)
    }

    /// 读取可空 decimal；参数为列下标，返回转换值或驱动错误。
    /// 对应 Java: java.sql.ResultSet#getBigDecimal(int)。
    fn big_decimal(&self, column: ColumnIndex) -> Result<Option<BigDecimal>, RdbcError> {
        decimal_object(self.read_object(column, &ResultSetRead::BigDecimal(None))?)
    }

    /// 按列名读取可空 decimal；保留驱动列名入口。
    /// 对应 Java: java.sql.ResultSet#getBigDecimal(String)。
    fn big_decimal_by_label(&self, label: &str) -> Result<Option<BigDecimal>, RdbcError> {
        decimal_object(self.read_object_by_label(label, &ResultSetRead::BigDecimal(None))?)
    }

    /// 按列和原 scale 读取 decimal，返回转换值或驱动错误。
    /// 对应 Java: java.sql.ResultSet#getBigDecimal(int,int)。
    fn big_decimal_with_scale(
        &self,
        column: ColumnIndex,
        scale: i32,
    ) -> Result<Option<BigDecimal>, RdbcError> {
        decimal_object(self.read_object(column, &ResultSetRead::BigDecimal(Some(scale)))?)
    }

    /// 按列名和原 scale 读取 decimal，返回转换值或驱动错误。
    /// 对应 Java: java.sql.ResultSet#getBigDecimal(String,int)。
    fn big_decimal_by_label_with_scale(
        &self,
        label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, RdbcError> {
        decimal_object(self.read_object_by_label(label, &ResultSetRead::BigDecimal(Some(scale)))?)
    }

    /// 读取可空 Date；参数为列和完整 Calendar 重载状态，返回值或错误。
    /// 对应 Java: java.sql.ResultSet#getDate(int, Calendar)。
    fn date(
        &self,
        column: ColumnIndex,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, RdbcError> {
        date_object(self.read_object(column, &ResultSetRead::Date(calendar.clone()))?)
    }

    /// 按列名读取可空 Date，保留完整 Calendar 参数；返回值或错误。
    /// 对应 Java: java.sql.ResultSet#getDate(String, Calendar)。
    fn date_by_label(
        &self,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, RdbcError> {
        date_object(self.read_object_by_label(label, &ResultSetRead::Date(calendar.clone()))?)
    }

    /// 读取可空 Time；参数为列和完整 Calendar 重载状态，返回值或错误。
    /// 对应 Java: java.sql.ResultSet#getTime(int, Calendar)。
    fn time(
        &self,
        column: ColumnIndex,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, RdbcError> {
        time_object(self.read_object(column, &ResultSetRead::Time(calendar.clone()))?)
    }

    /// 按列名读取可空 Time，保留完整 Calendar 参数；返回值或错误。
    /// 对应 Java: java.sql.ResultSet#getTime(String, Calendar)。
    fn time_by_label(
        &self,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, RdbcError> {
        time_object(self.read_object_by_label(label, &ResultSetRead::Time(calendar.clone()))?)
    }

    /// 读取可空 Timestamp；参数为列和完整 Calendar 重载状态，返回值或错误。
    /// 对应 Java: java.sql.ResultSet#getTimestamp(int, Calendar)。
    fn timestamp(
        &self,
        column: ColumnIndex,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, RdbcError> {
        timestamp_object(self.read_object(column, &ResultSetRead::Timestamp(calendar.clone()))?)
    }

    /// 按列名读取可空 Timestamp，保留完整 Calendar 参数；返回值或错误。
    /// 对应 Java: java.sql.ResultSet#getTimestamp(String, Calendar)。
    fn timestamp_by_label(
        &self,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, RdbcError> {
        timestamp_object(
            self.read_object_by_label(label, &ResultSetRead::Timestamp(calendar.clone()))?,
        )
    }

    /// 使用 type map 按 label 读取 object。
    fn object_by_label_with_type_map(
        &self,
        label: &str,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.object_with_type_map_by_label(label, type_map)
    }

    /// 使用 target type 按 label 读取 object。
    fn object_by_label_as(
        &self,
        label: &str,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.object_as_by_label(label, target_type)
    }

    /// 读取 Ref handle。
    fn reference(&self, column: ColumnIndex) -> Result<Option<RdbcRef>, RdbcError> {
        object_variant(
            self.read_object(column, &ResultSetRead::Ref)?,
            "Ref",
            |value| match value {
                RdbcObject::Ref(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 按 label 读取 Ref handle。
    fn reference_by_label(&self, label: &str) -> Result<Option<RdbcRef>, RdbcError> {
        object_variant(
            self.read_object_by_label(label, &ResultSetRead::Ref)?,
            "Ref",
            |value| match value {
                RdbcObject::Ref(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 Blob handle。
    fn blob(&self, column: ColumnIndex) -> Result<Option<RdbcBlob>, RdbcError> {
        object_variant(
            self.read_object(column, &ResultSetRead::Blob)?,
            "Blob",
            |value| match value {
                RdbcObject::Blob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 按 label 读取 Blob handle。
    fn blob_by_label(&self, label: &str) -> Result<Option<RdbcBlob>, RdbcError> {
        object_variant(
            self.read_object_by_label(label, &ResultSetRead::Blob)?,
            "Blob",
            |value| match value {
                RdbcObject::Blob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 Clob handle。
    fn clob(&self, column: ColumnIndex) -> Result<Option<RdbcClob>, RdbcError> {
        object_variant(
            self.read_object(column, &ResultSetRead::Clob)?,
            "Clob",
            |value| match value {
                RdbcObject::Clob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 按 label 读取 Clob handle。
    fn clob_by_label(&self, label: &str) -> Result<Option<RdbcClob>, RdbcError> {
        object_variant(
            self.read_object_by_label(label, &ResultSetRead::Clob)?,
            "Clob",
            |value| match value {
                RdbcObject::Clob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 Array handle。
    fn array(&self, column: ColumnIndex) -> Result<Option<RdbcArray>, RdbcError> {
        object_variant(
            self.read_object(column, &ResultSetRead::Array)?,
            "Array",
            |value| match value {
                RdbcObject::Array(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 按 label 读取 Array handle。
    fn array_by_label(&self, label: &str) -> Result<Option<RdbcArray>, RdbcError> {
        object_variant(
            self.read_object_by_label(label, &ResultSetRead::Array)?,
            "Array",
            |value| match value {
                RdbcObject::Array(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 URL value。
    fn url(&self, column: ColumnIndex) -> Result<Option<RdbcUrl>, RdbcError> {
        object_variant(
            self.read_object(column, &ResultSetRead::Url)?,
            "URL",
            |value| match value {
                RdbcObject::Url(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 按 label 读取 URL value。
    fn url_by_label(&self, label: &str) -> Result<Option<RdbcUrl>, RdbcError> {
        object_variant(
            self.read_object_by_label(label, &ResultSetRead::Url)?,
            "URL",
            |value| match value {
                RdbcObject::Url(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 RowId value。
    fn row_id(&self, column: ColumnIndex) -> Result<Option<RdbcRowId>, RdbcError> {
        object_variant(
            self.read_object(column, &ResultSetRead::RowId)?,
            "RowId",
            |value| match value {
                RdbcObject::RowId(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 按 label 读取 RowId value。
    fn row_id_by_label(&self, label: &str) -> Result<Option<RdbcRowId>, RdbcError> {
        object_variant(
            self.read_object_by_label(label, &ResultSetRead::RowId)?,
            "RowId",
            |value| match value {
                RdbcObject::RowId(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 NClob handle。
    fn n_clob(&self, column: ColumnIndex) -> Result<Option<RdbcNClob>, RdbcError> {
        object_variant(
            self.read_object(column, &ResultSetRead::NClob)?,
            "NClob",
            |value| match value {
                RdbcObject::NClob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 按 label 读取 NClob handle。
    fn n_clob_by_label(&self, label: &str) -> Result<Option<RdbcNClob>, RdbcError> {
        object_variant(
            self.read_object_by_label(label, &ResultSetRead::NClob)?,
            "NClob",
            |value| match value {
                RdbcObject::NClob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 SQLXML handle。
    fn sql_xml(&self, column: ColumnIndex) -> Result<Option<RdbcSqlXml>, RdbcError> {
        object_variant(
            self.read_object(column, &ResultSetRead::SqlXml)?,
            "SQLXML",
            |value| match value {
                RdbcObject::SqlXml(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 按 label 读取 SQLXML handle。
    fn sql_xml_by_label(&self, label: &str) -> Result<Option<RdbcSqlXml>, RdbcError> {
        object_variant(
            self.read_object_by_label(label, &ResultSetRead::SqlXml)?,
            "SQLXML",
            |value| match value {
                RdbcObject::SqlXml(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 ASCII stream。
    fn ascii_stream(&self, column: ColumnIndex) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream(column, crate::result_set::ResultSetStreamKind::Ascii)
    }

    /// 按 label 读取 ASCII stream。
    fn ascii_stream_by_label(&self, label: &str) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream_by_label(label, crate::result_set::ResultSetStreamKind::Ascii)
    }

    /// 读取 legacy Unicode stream。
    fn unicode_stream(&self, column: ColumnIndex) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream(column, crate::result_set::ResultSetStreamKind::Unicode)
    }

    /// 按 label 读取 legacy Unicode stream。
    fn unicode_stream_by_label(&self, label: &str) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream_by_label(label, crate::result_set::ResultSetStreamKind::Unicode)
    }

    /// 读取 binary stream。
    fn binary_stream(&self, column: ColumnIndex) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream(column, crate::result_set::ResultSetStreamKind::Binary)
    }

    /// 按 label 读取 binary stream。
    fn binary_stream_by_label(&self, label: &str) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.input_stream_by_label(label, crate::result_set::ResultSetStreamKind::Binary)
    }

    /// 读取 character Reader。
    fn character_stream(&self, column: ColumnIndex) -> Result<Option<RdbcReader>, RdbcError> {
        self.reader(column, false)
    }

    /// 按 label 读取 character Reader。
    fn character_stream_by_label(&self, label: &str) -> Result<Option<RdbcReader>, RdbcError> {
        self.reader_by_label(label, false)
    }

    /// 读取 national-character Reader。
    fn n_character_stream(&self, column: ColumnIndex) -> Result<Option<RdbcReader>, RdbcError> {
        self.reader(column, true)
    }

    /// 按 label 读取 national-character Reader。
    fn n_character_stream_by_label(&self, label: &str) -> Result<Option<RdbcReader>, RdbcError> {
        self.reader_by_label(label, true)
    }

    /// 更新 Ref handle。
    fn update_reference(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcRef>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, ResultSetUpdate::Ref(value))
    }

    /// 按 label 更新 Ref handle。
    fn update_reference_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcRef>,
    ) -> Result<(), RdbcError> {
        self.update_value_by_label(label, ResultSetUpdate::Ref(value))
    }

    /// 更新 Blob handle。
    fn update_blob(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcBlob>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, ResultSetUpdate::Blob(value))
    }

    /// 按 label 更新 Blob handle。
    fn update_blob_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcBlob>,
    ) -> Result<(), RdbcError> {
        self.update_value_by_label(label, ResultSetUpdate::Blob(value))
    }

    /// 更新 Clob handle。
    fn update_clob(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcClob>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, ResultSetUpdate::Clob(value))
    }

    /// 按 label 更新 Clob handle。
    fn update_clob_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcClob>,
    ) -> Result<(), RdbcError> {
        self.update_value_by_label(label, ResultSetUpdate::Clob(value))
    }

    /// 更新 Array handle。
    fn update_array(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcArray>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, ResultSetUpdate::Array(value))
    }

    /// 按 label 更新 Array handle。
    fn update_array_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcArray>,
    ) -> Result<(), RdbcError> {
        self.update_value_by_label(label, ResultSetUpdate::Array(value))
    }

    /// 更新 RowId value。
    fn update_row_id(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcRowId>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, ResultSetUpdate::RowId(value))
    }

    /// 按 label 更新 RowId value。
    fn update_row_id_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcRowId>,
    ) -> Result<(), RdbcError> {
        self.update_value_by_label(label, ResultSetUpdate::RowId(value))
    }

    /// 更新 NClob handle。
    fn update_n_clob(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcNClob>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, ResultSetUpdate::NClob(value))
    }

    /// 按 label 更新 NClob handle。
    fn update_n_clob_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcNClob>,
    ) -> Result<(), RdbcError> {
        self.update_value_by_label(label, ResultSetUpdate::NClob(value))
    }

    /// 更新 SQLXML handle。
    fn update_sql_xml(
        &mut self,
        column: ColumnIndex,
        value: Option<RdbcSqlXml>,
    ) -> Result<(), RdbcError> {
        self.update_value(column, ResultSetUpdate::SqlXml(value))
    }

    /// 按 label 更新 SQLXML handle。
    fn update_sql_xml_by_label(
        &mut self,
        label: &str,
        value: Option<RdbcSqlXml>,
    ) -> Result<(), RdbcError> {
        self.update_value_by_label(label, ResultSetUpdate::SqlXml(value))
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
            ResultSetUpdate::BlobStream {
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
        self.update_value_by_label(
            label,
            ResultSetUpdate::BlobStream {
                stream: stream.cloned(),
                length,
            },
        )
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
            ResultSetUpdate::ClobReader {
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
        self.update_value_by_label(
            label,
            ResultSetUpdate::ClobReader {
                reader: reader.cloned(),
                length,
            },
        )
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
            ResultSetUpdate::NClobReader {
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
        self.update_value_by_label(
            label,
            ResultSetUpdate::NClobReader {
                reader: reader.cloned(),
                length,
            },
        )
    }
}

impl<T: ResultSet + ?Sized> ResultSetExt for T {}

// 所有转换仅检查单次标准读取结果，不递归调用其他 getter，保留原 NULL 与类型错误规则。
fn scalar_value(object: RdbcObject, expected: &str) -> Result<Value, RdbcError> {
    match object {
        RdbcObject::Scalar(value) => Ok(value),
        other => Err(result_set_type_error(expected, &other)),
    }
}

fn string_object(object: RdbcObject) -> Result<Option<String>, RdbcError> {
    match scalar_value(object, "String")? {
        Value::Null => Ok(None),
        Value::String(value) => Ok(Some(value)),
        other => Err(result_set_type_error("String", &RdbcObject::Scalar(other))),
    }
}

fn boolean_object(object: RdbcObject) -> Result<bool, RdbcError> {
    match scalar_value(object, "boolean")? {
        Value::Null => Ok(false),
        Value::Bool(value) => Ok(value),
        Value::Int(value) => Ok(value != 0),
        Value::Float(value) => Ok(value != 0.0),
        Value::Decimal(value) => Ok(value != 0),
        Value::String(value) => Ok(value == "1" || value.eq_ignore_ascii_case("true")),
        other => Err(result_set_type_error("boolean", &RdbcObject::Scalar(other))),
    }
}

fn long_object(object: RdbcObject) -> Result<i64, RdbcError> {
    integer_value(scalar_value(object, "long")?, "long")
}

fn int_object(object: RdbcObject) -> Result<i32, RdbcError> {
    i32::try_from(long_object(object)?)
        .map_err(|_| RdbcError::DriverError("ResultSet value exceeds int".to_owned()))
}

fn short_object(object: RdbcObject) -> Result<i16, RdbcError> {
    i16::try_from(long_object(object)?)
        .map_err(|_| RdbcError::DriverError("ResultSet value exceeds short".to_owned()))
}

fn byte_object(object: RdbcObject) -> Result<i8, RdbcError> {
    i8::try_from(long_object(object)?)
        .map_err(|_| RdbcError::DriverError("ResultSet value exceeds byte".to_owned()))
}

fn double_object(object: RdbcObject) -> Result<f64, RdbcError> {
    match scalar_value(object, "double")? {
        Value::Null => Ok(0.0),
        Value::Float(value) => Ok(value),
        other => Err(result_set_type_error("double", &RdbcObject::Scalar(other))),
    }
}

#[allow(clippy::cast_possible_truncation)]
fn float_object(object: RdbcObject) -> Result<f32, RdbcError> {
    Ok(double_object(object)? as f32)
}

fn bytes_object(object: RdbcObject) -> Result<Option<Vec<u8>>, RdbcError> {
    match scalar_value(object, "bytes")? {
        Value::Null => Ok(None),
        Value::Bytes(value) => Ok(Some(value)),
        other => Err(result_set_type_error("bytes", &RdbcObject::Scalar(other))),
    }
}

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

/// 将标量或原生对象转换为 decimal；参数为对象，返回可空值或转换错误。
/// 对应 Java: java.sql.ResultSet#getBigDecimal 的默认标量转换。
pub(crate) fn decimal_object(object: RdbcObject) -> Result<Option<BigDecimal>, RdbcError> {
    match object {
        RdbcObject::Scalar(Value::Null) => Ok(None),
        RdbcObject::BigDecimal(value) | RdbcObject::Scalar(Value::Decimal(value)) => {
            Ok(Some(value))
        }
        RdbcObject::Scalar(Value::Int(value)) => Ok(Some(BigDecimal::from(value))),
        RdbcObject::Scalar(Value::Float(value)) => {
            decimal_from_f64(value).map(Some).ok_or_else(|| {
                RdbcError::DriverError(format!(
                    "ResultSet floating point value {value} cannot be represented as BigDecimal"
                ))
            })
        }
        RdbcObject::Scalar(Value::String(value)) => {
            value.parse::<BigDecimal>().map(Some).map_err(|error| {
                RdbcError::DriverError(format!(
                    "ResultSet value {value:?} cannot be converted to BigDecimal: {error}"
                ))
            })
        }
        other => Err(result_set_type_error("BigDecimal", &other)),
    }
}

/// 校验默认资源读取的对象家族；参数为驱动对象及请求，返回原资源、NULL或准确不支持错误。
/// 对应 Java: java.sql.ResultSet#getRef/getBlob/getClob/getArray/getURL/getRowId/getNClob/getSQLXML。
pub(crate) fn resource_object(
    object: RdbcObject,
    request: &ResultSetRead,
) -> Result<RdbcObject, RdbcError> {
    if std::matches!(&object, RdbcObject::Scalar(Value::Null)) {
        return Ok(object);
    }
    let (supported, operation) = match request {
        ResultSetRead::Ref => (std::matches!(&object, RdbcObject::Ref(_)), "result_set_ref"),
        ResultSetRead::Blob => (
            std::matches!(&object, RdbcObject::Blob(_)),
            "result_set_blob",
        ),
        ResultSetRead::Clob => (
            std::matches!(&object, RdbcObject::Clob(_)),
            "result_set_clob",
        ),
        ResultSetRead::Array => (
            std::matches!(&object, RdbcObject::Array(_)),
            "result_set_array",
        ),
        ResultSetRead::Url => (std::matches!(&object, RdbcObject::Url(_)), "result_set_url"),
        ResultSetRead::RowId => (
            std::matches!(&object, RdbcObject::RowId(_)),
            "result_set_row_id",
        ),
        ResultSetRead::NClob => (
            std::matches!(&object, RdbcObject::NClob(_)),
            "result_set_n_clob",
        ),
        ResultSetRead::SqlXml => (
            std::matches!(&object, RdbcObject::SqlXml(_)),
            "result_set_sql_xml",
        ),
        ResultSetRead::String
        | ResultSetRead::NString
        | ResultSetRead::Boolean
        | ResultSetRead::Long
        | ResultSetRead::Int
        | ResultSetRead::Short
        | ResultSetRead::Byte
        | ResultSetRead::Double
        | ResultSetRead::Float
        | ResultSetRead::Bytes
        | ResultSetRead::BigDecimal(_)
        | ResultSetRead::Date(_)
        | ResultSetRead::Time(_)
        | ResultSetRead::Timestamp(_) => return Ok(object),
    };
    if supported {
        Ok(object)
    } else {
        Err(RdbcError::UnsupportedOperation { operation })
    }
}

/// 将日期、时间戳或日期字符串转换为日期；参数为对象，返回可空日期或错误。
/// 对应 Java: java.sql.ResultSet#getDate 的默认标量转换。
pub(crate) fn date_object(object: RdbcObject) -> Result<Option<NaiveDate>, RdbcError> {
    match object {
        RdbcObject::Scalar(Value::Null) => Ok(None),
        RdbcObject::Date(value) | RdbcObject::Scalar(Value::Date(value)) => Ok(Some(value)),
        RdbcObject::Timestamp(value) | RdbcObject::Scalar(Value::Timestamp(value)) => {
            Ok(Some(value.date()))
        }
        RdbcObject::Scalar(Value::String(value)) => NaiveDate::parse_from_str(&value, "%Y-%m-%d")
            .map(Some)
            .map_err(|error| {
                RdbcError::DriverError(format!(
                    "ResultSet value {value:?} cannot be converted to Date: {error}"
                ))
            }),
        other => Err(result_set_type_error("Date", &other)),
    }
}

/// 将时间、时间戳或时间字符串转换为时间；参数为对象，返回可空时间或错误。
/// 对应 Java: java.sql.ResultSet#getTime 的默认标量转换。
pub(crate) fn time_object(object: RdbcObject) -> Result<Option<NaiveTime>, RdbcError> {
    match object {
        RdbcObject::Scalar(Value::Null) => Ok(None),
        RdbcObject::Time(value) | RdbcObject::Scalar(Value::Time(value)) => Ok(Some(value)),
        RdbcObject::Timestamp(value) | RdbcObject::Scalar(Value::Timestamp(value)) => {
            Ok(Some(value.time()))
        }
        RdbcObject::Scalar(Value::String(value)) => {
            NaiveTime::parse_from_str(&value, "%H:%M:%S%.f")
                .map(Some)
                .map_err(|error| {
                    RdbcError::DriverError(format!(
                        "ResultSet value {value:?} cannot be converted to Time: {error}"
                    ))
                })
        }
        other => Err(result_set_type_error("Time", &other)),
    }
}

/// 将时间戳、日期或支持的字符串转换为时间戳；日期使用午夜，返回可空值或错误。
/// 对应 Java: java.sql.ResultSet#getTimestamp 的默认标量转换。
pub(crate) fn timestamp_object(object: RdbcObject) -> Result<Option<NaiveDateTime>, RdbcError> {
    match object {
        RdbcObject::Scalar(Value::Null) => Ok(None),
        RdbcObject::Timestamp(value) | RdbcObject::Scalar(Value::Timestamp(value)) => {
            Ok(Some(value))
        }
        RdbcObject::Date(value) | RdbcObject::Scalar(Value::Date(value)) => {
            Ok(Some(NaiveDateTime::new(value, NaiveTime::MIN)))
        }
        RdbcObject::Scalar(Value::String(value)) => {
            NaiveDateTime::parse_from_str(&value, "%Y-%m-%d %H:%M:%S%.f")
                .or_else(|_| NaiveDateTime::parse_from_str(&value, "%Y-%m-%dT%H:%M:%S%.f"))
                .map(Some)
                .map_err(|error| {
                    RdbcError::DriverError(format!(
                        "ResultSet value {value:?} cannot be converted to Timestamp: {error}"
                    ))
                })
        }
        other => Err(result_set_type_error("Timestamp", &other)),
    }
}

/// 保留默认 national 字符串的严格类型规则；参数为对象，返回可空 Unicode 或类型错误。
/// 对应 Java: java.sql.ResultSet#getNString 的默认对象读取。
pub(crate) fn n_string_object(object: RdbcObject) -> Result<Option<String>, RdbcError> {
    match object {
        RdbcObject::Scalar(Value::Null) => Ok(None),
        RdbcObject::NString(value) | RdbcObject::Scalar(Value::String(value)) => Ok(Some(value)),
        other => Err(result_set_type_error("NString", &other)),
    }
}

pub(crate) fn result_set_type_error(expected: &str, actual: &RdbcObject) -> RdbcError {
    RdbcError::DriverError(format!(
        "ResultSet expected {expected} column value, got {actual}"
    ))
}

/// 将单次读取的 SQL 标量转换为指定目标；参数为原值和目标，返回对象或错误。
/// 对应 Java: java.sql.ResultSet#getObject(int, Class) 的默认值转换。
pub(crate) fn typed_scalar_object(
    value: Value,
    target: &RdbcTargetType,
) -> Result<RdbcObject, RdbcError> {
    if std::matches!(value, Value::Null) {
        return Ok(RdbcObject::Scalar(Value::Null));
    }
    match target {
        RdbcTargetType::String => typed_string(value).map(RdbcObject::String),
        RdbcTargetType::Boolean => {
            let boolean = match value {
                Value::Bool(value) => value,
                Value::Int(value) => value != 0,
                Value::Float(value) => value != 0.0,
                Value::Decimal(value) => value != 0,
                Value::String(value) => value == "1" || value.eq_ignore_ascii_case("true"),
                actual => return Err(typed_value_error("Boolean", &actual)),
            };
            Ok(RdbcObject::Boolean(boolean))
        }
        RdbcTargetType::Byte => i8::try_from(typed_long(value)?)
            .map(RdbcObject::Byte)
            .map_err(|error| RdbcError::DriverError(error.to_string())),
        RdbcTargetType::Short => i16::try_from(typed_long(value)?)
            .map(RdbcObject::Short)
            .map_err(|error| RdbcError::DriverError(error.to_string())),
        RdbcTargetType::Integer => i32::try_from(typed_long(value)?)
            .map(RdbcObject::Integer)
            .map_err(|error| RdbcError::DriverError(error.to_string())),
        RdbcTargetType::Long => typed_long(value).map(RdbcObject::Long),
        RdbcTargetType::Float => typed_double(value).map(|value| RdbcObject::Float(value as f32)),
        RdbcTargetType::Double => typed_double(value).map(RdbcObject::Double),
        RdbcTargetType::Bytes => match value {
            Value::Bytes(value) => Ok(RdbcObject::Bytes(value)),
            other => typed_string(other).map(|value| RdbcObject::Bytes(value.into_bytes())),
        },
        RdbcTargetType::BigDecimal => decimal_object(RdbcObject::Scalar(value))?
            .map(RdbcObject::BigDecimal)
            .ok_or_else(typed_unexpected_null),
        RdbcTargetType::Date => date_object(RdbcObject::Scalar(value))?
            .map(RdbcObject::Date)
            .ok_or_else(typed_unexpected_null),
        RdbcTargetType::Time => time_object(RdbcObject::Scalar(value))?
            .map(RdbcObject::Time)
            .ok_or_else(typed_unexpected_null),
        RdbcTargetType::Timestamp => timestamp_object(RdbcObject::Scalar(value))?
            .map(RdbcObject::Timestamp)
            .ok_or_else(typed_unexpected_null),
        _ => Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_typed_resource",
        }),
    }
}

// 保留 BASE 原标量转换规则；不重新读取游标，也不调用可能覆盖的其他标量 getter。
fn typed_string(value: Value) -> Result<String, RdbcError> {
    match value {
        Value::String(value) => Ok(value),
        Value::Bytes(value) => {
            String::from_utf8(value).map_err(|error| RdbcError::DriverError(error.to_string()))
        }
        Value::Bool(value) => Ok(value.to_string()),
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(value.to_string()),
        Value::Decimal(value) => Ok(value.to_string()),
        Value::Date(value) => Ok(value.to_string()),
        Value::Time(value) => Ok(value.to_string()),
        Value::Timestamp(value) => Ok(value.to_string()),
        Value::Null => Err(typed_unexpected_null()),
    }
}

fn typed_long(value: Value) -> Result<i64, RdbcError> {
    match value {
        Value::Bool(value) => Ok(i64::from(value)),
        Value::Int(value) => Ok(value),
        Value::Float(value) => Ok(value as i64),
        Value::Decimal(value) => {
            decimal_to_i64(&value).ok_or_else(|| typed_value_error("Long", &Value::Decimal(value)))
        }
        Value::String(value) => value
            .parse::<i64>()
            .map_err(|error| RdbcError::DriverError(error.to_string())),
        actual => Err(typed_value_error("Long", &actual)),
    }
}

fn typed_double(value: Value) -> Result<f64, RdbcError> {
    match value {
        Value::Bool(value) => Ok(if value { 1.0 } else { 0.0 }),
        Value::Int(value) => Ok(value as f64),
        Value::Float(value) => Ok(value),
        Value::Decimal(value) => decimal_to_f64(&value)
            .ok_or_else(|| typed_value_error("Double", &Value::Decimal(value))),
        Value::String(value) => value
            .parse::<f64>()
            .map_err(|error| RdbcError::DriverError(error.to_string())),
        actual => Err(typed_value_error("Double", &actual)),
    }
}

fn typed_unexpected_null() -> RdbcError {
    RdbcError::DriverError("unexpected SQL NULL".to_owned())
}

fn typed_value_error(expected: &str, actual: &Value) -> RdbcError {
    RdbcError::DriverError(format!("ResultSet expected {expected} value, got {actual}"))
}
