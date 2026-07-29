//! 物理 JDBC `ResultSet` 资源句柄。
//!
//! 对应 Java 平台对象：`java.sql.ResultSet`。本对象先承载 Array 返回行集所需的
//! 身份和关闭生命周期；游标、metadata 与 pooled trace 继续由 ResultSet 专项迁移。

use super::{
    DruidError, JdbcArray, JdbcBlob, JdbcCalendarArgument, JdbcCharacterLength, JdbcClob,
    JdbcInputStream, JdbcNClob, JdbcObject, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml,
    JdbcStreamLength, JdbcTargetType, JdbcTypeMap, JdbcUrl, ResultSetColumnMeta,
    ResultSetColumnType, ResultSetMetaData, ResultSetUpdate, Row, SqlWarning, Value,
};
use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

/// 物理结果集最小资源 SPI。
pub trait PhysicalResultSet: fmt::Debug + Send + Sync {
    /// 关闭物理结果集。
    fn close(&self) -> Result<(), DruidError>;

    /// 返回结果集是否关闭。
    fn is_closed(&self) -> bool;

    /// 移到下一行。
    fn next(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_next",
        })
    }

    /// 移到上一行。
    fn previous(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_previous",
        })
    }

    /// 移到第一行。
    fn first(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_first",
        })
    }

    /// 移到最后一行。
    fn last(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_last",
        })
    }

    /// 移到第一行之前。
    fn before_first(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_before_first",
        })
    }

    /// 移到最后一行之后。
    fn after_last(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_after_last",
        })
    }

    /// 按 JDBC 绝对行号定位。
    fn absolute(&self, _row: i32) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_absolute",
        })
    }

    /// 相对当前游标定位。
    fn relative(&self, _rows: i32) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_relative",
        })
    }

    /// 返回当前 JDBC 行号；无当前行时为 0。
    fn row(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_row",
        })
    }

    /// 返回当前行指定 1-based 列值。
    fn value(&self, _column_index: usize) -> Result<Value, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_value",
        })
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getString(int)`。
    ///
    /// 真实驱动可覆盖本方法，保留 vendor 转换规则；默认实现仅为基于
    /// `Value` 的 Adapter 提供等价回退。
    fn string(&self, column_index: usize) -> Result<Option<String>, DruidError> {
        match self.value(column_index)? {
            Value::Null => Ok(None),
            value => value_to_string(value).map(Some),
        }
    }

    /// 按标签执行 JDBC `ResultSet#getString(String)`。
    fn string_by_label(&self, column_label: &str) -> Result<Option<String>, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.string(column_index)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getNString(int)`。
    ///
    /// Rust 字符串统一为 Unicode，但仍保留独立 SPI，避免驱动和 Filter 无法
    /// 区分 JDBC 的 `getString` 与 `getNString` 调用。
    fn n_string(&self, column_index: usize) -> Result<Option<String>, DruidError> {
        self.string(column_index)
    }

    /// 按标签执行 JDBC `ResultSet#getNString(String)`。
    fn n_string_by_label(&self, column_label: &str) -> Result<Option<String>, DruidError> {
        self.string_by_label(column_label)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getBoolean(int)`。
    fn boolean(&self, column_index: usize) -> Result<bool, DruidError> {
        match self.value(column_index)? {
            Value::Null => Ok(false),
            value => value_to_boolean(value),
        }
    }

    /// 按标签执行 JDBC `ResultSet#getBoolean(String)`。
    fn boolean_by_label(&self, column_label: &str) -> Result<bool, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.boolean(column_index)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getLong(int)`。
    fn long(&self, column_index: usize) -> Result<i64, DruidError> {
        match self.value(column_index)? {
            Value::Null => Ok(0),
            value => value_to_long(value),
        }
    }

    /// 按标签执行 JDBC `ResultSet#getLong(String)`。
    fn long_by_label(&self, column_label: &str) -> Result<i64, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.long(column_index)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getInt(int)`。
    fn int(&self, column_index: usize) -> Result<i32, DruidError> {
        i32::try_from(self.long(column_index)?)
            .map_err(|error| DruidError::DriverError(error.to_string()))
    }

    /// 按标签执行 JDBC `ResultSet#getInt(String)`。
    fn int_by_label(&self, column_label: &str) -> Result<i32, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.int(column_index)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getShort(int)`。
    fn short(&self, column_index: usize) -> Result<i16, DruidError> {
        i16::try_from(self.long(column_index)?)
            .map_err(|error| DruidError::DriverError(error.to_string()))
    }

    /// 按标签执行 JDBC `ResultSet#getShort(String)`。
    fn short_by_label(&self, column_label: &str) -> Result<i16, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.short(column_index)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getByte(int)`。
    fn byte(&self, column_index: usize) -> Result<i8, DruidError> {
        i8::try_from(self.long(column_index)?)
            .map_err(|error| DruidError::DriverError(error.to_string()))
    }

    /// 按标签执行 JDBC `ResultSet#getByte(String)`。
    fn byte_by_label(&self, column_label: &str) -> Result<i8, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.byte(column_index)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getDouble(int)`。
    fn double(&self, column_index: usize) -> Result<f64, DruidError> {
        match self.value(column_index)? {
            Value::Null => Ok(0.0),
            value => value_to_double(value),
        }
    }

    /// 按标签执行 JDBC `ResultSet#getDouble(String)`。
    fn double_by_label(&self, column_label: &str) -> Result<f64, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.double(column_index)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getFloat(int)`。
    fn float(&self, column_index: usize) -> Result<f32, DruidError> {
        Ok(self.double(column_index)? as f32)
    }

    /// 按标签执行 JDBC `ResultSet#getFloat(String)`。
    fn float_by_label(&self, column_label: &str) -> Result<f32, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.float(column_index)
    }

    /// 按 1-based 下标执行 JDBC `ResultSet#getBytes(int)`。
    fn bytes(&self, column_index: usize) -> Result<Option<Vec<u8>>, DruidError> {
        match self.value(column_index)? {
            Value::Null => Ok(None),
            value => value_to_bytes(value).map(Some),
        }
    }

    /// 按标签执行 JDBC `ResultSet#getBytes(String)`。
    fn bytes_by_label(&self, column_label: &str) -> Result<Option<Vec<u8>>, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.bytes(column_index)
    }

    /// 按标签执行 JDBC `ResultSet#getObject(String)` 的通用值分支。
    fn value_by_label(&self, column_label: &str) -> Result<Value, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.value(column_index)
    }

    /// 按 1-based 下标读取任意精度 Decimal。
    ///
    /// 对应 Java：`ResultSet#getBigDecimal(int)` 和已废弃的
    /// `getBigDecimal(int, int)`；`scale=None` 表示无 scale 重载。
    fn big_decimal(
        &self,
        column_index: usize,
        scale: Option<i32>,
    ) -> Result<Option<BigDecimal>, DruidError> {
        value_to_big_decimal(self.value(column_index)?, scale)
    }

    /// 按标签读取任意精度 Decimal。
    ///
    /// 独立保留标签重载，使真实驱动可以覆盖其原生标签解析规则。
    fn big_decimal_by_label(
        &self,
        column_label: &str,
        scale: Option<i32>,
    ) -> Result<Option<BigDecimal>, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.big_decimal(column_index, scale)
    }

    /// 按 1-based 下标读取 DATE。
    ///
    /// `calendar` 区分无 Calendar 重载、显式 null Calendar 和具体时区。
    fn date(
        &self,
        column_index: usize,
        _calendar: &JdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        value_to_date(self.value(column_index)?)
    }

    /// 按标签读取 DATE。
    fn date_by_label(
        &self,
        column_label: &str,
        calendar: &JdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.date(column_index, calendar)
    }

    /// 按 1-based 下标读取 TIME。
    fn time(
        &self,
        column_index: usize,
        _calendar: &JdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        value_to_time(self.value(column_index)?)
    }

    /// 按标签读取 TIME。
    fn time_by_label(
        &self,
        column_label: &str,
        calendar: &JdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.time(column_index, calendar)
    }

    /// 按 1-based 下标读取 TIMESTAMP。
    fn timestamp(
        &self,
        column_index: usize,
        _calendar: &JdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        value_to_timestamp(self.value(column_index)?)
    }

    /// 按标签读取 TIMESTAMP。
    fn timestamp_by_label(
        &self,
        column_label: &str,
        calendar: &JdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.timestamp(column_index, calendar)
    }

    /// 按 1-based 下标和显式 SQL 类型映射读取对象。
    ///
    /// 对应 Java：`ResultSet#getObject(int, Map<String, Class<?>>)`；
    /// `None` 保留 Java `null` Map。
    fn object_with_type_map(
        &self,
        _column_index: usize,
        _type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_get_object_with_type_map",
        })
    }

    /// 按标签和显式 SQL 类型映射读取对象。
    fn object_by_label_with_type_map(
        &self,
        _column_label: &str,
        _type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_get_object_by_label_with_type_map",
        })
    }

    /// 按 1-based 下标和 Java `Class<T>` 对应目标类型读取对象。
    ///
    /// 目标类型原样到达物理 SPI；默认实现转换通用 `Value` 并委派标准资源
    /// getter。vendor custom 类型必须由真实驱动覆盖。
    fn object_as(
        &self,
        column_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        match target_type {
            JdbcTargetType::Blob => Ok(self
                .blob(column_index)?
                .map_or_else(|| JdbcObject::Scalar(Value::Null), JdbcObject::Blob)),
            JdbcTargetType::Clob => Ok(self
                .clob(column_index)?
                .map_or_else(|| JdbcObject::Scalar(Value::Null), JdbcObject::Clob)),
            JdbcTargetType::NClob => Ok(self
                .n_clob(column_index)?
                .map_or_else(|| JdbcObject::Scalar(Value::Null), JdbcObject::NClob)),
            JdbcTargetType::Array => Ok(self
                .array(column_index)?
                .map_or_else(|| JdbcObject::Scalar(Value::Null), JdbcObject::Array)),
            JdbcTargetType::Ref => Ok(self
                .reference(column_index)?
                .map_or_else(|| JdbcObject::Scalar(Value::Null), JdbcObject::Ref)),
            JdbcTargetType::RowId => Ok(self
                .row_id(column_index)?
                .map_or_else(|| JdbcObject::Scalar(Value::Null), JdbcObject::RowId)),
            JdbcTargetType::SqlXml => Ok(self
                .sql_xml(column_index)?
                .map_or_else(|| JdbcObject::Scalar(Value::Null), JdbcObject::SqlXml)),
            JdbcTargetType::Url => Ok(self
                .url(column_index)?
                .map_or_else(|| JdbcObject::Scalar(Value::Null), JdbcObject::Url)),
            JdbcTargetType::Custom(_) => {
                // 先读取值以保持 JDBC 驱动对游标状态、列下标等前置条件的错误优先级。
                let _ = self.value(column_index)?;
                Err(DruidError::UnsupportedOperation {
                    operation: "result_set_get_object_typed_custom",
                })
            }
            _ => value_to_jdbc_object(self.value(column_index)?, target_type),
        }
    }

    /// 按标签和目标类型读取对象。
    fn object_by_label_as(
        &self,
        column_label: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        let column_index = self.find_column(column_label)?;
        self.object_as(column_index, target_type)
    }

    /// 按 1-based 下标读取 JDBC `Ref` 资源。
    fn reference(&self, _column_index: usize) -> Result<Option<JdbcRef>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_ref",
        })
    }

    /// 按标签读取 JDBC `Ref` 资源。
    fn reference_by_label(&self, _column_label: &str) -> Result<Option<JdbcRef>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_ref_by_label",
        })
    }

    /// 按 1-based 下标读取 JDBC `Blob` 资源。
    fn blob(&self, _column_index: usize) -> Result<Option<JdbcBlob>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_blob",
        })
    }

    /// 按标签读取 JDBC `Blob` 资源。
    fn blob_by_label(&self, _column_label: &str) -> Result<Option<JdbcBlob>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_blob_by_label",
        })
    }

    /// 按 1-based 下标读取 JDBC `Clob` 资源。
    fn clob(&self, _column_index: usize) -> Result<Option<JdbcClob>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_clob",
        })
    }

    /// 按标签读取 JDBC `Clob` 资源。
    fn clob_by_label(&self, _column_label: &str) -> Result<Option<JdbcClob>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_clob_by_label",
        })
    }

    /// 按 1-based 下标读取 JDBC `Array` 资源。
    fn array(&self, _column_index: usize) -> Result<Option<JdbcArray>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_array",
        })
    }

    /// 按标签读取 JDBC `Array` 资源。
    fn array_by_label(&self, _column_label: &str) -> Result<Option<JdbcArray>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_array_by_label",
        })
    }

    /// 按 1-based 下标读取 JDBC `URL` 值。
    fn url(&self, _column_index: usize) -> Result<Option<JdbcUrl>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_url",
        })
    }

    /// 按标签读取 JDBC `URL` 值。
    fn url_by_label(&self, _column_label: &str) -> Result<Option<JdbcUrl>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_url_by_label",
        })
    }

    /// 按 1-based 下标读取 JDBC `RowId` 值。
    fn row_id(&self, _column_index: usize) -> Result<Option<JdbcRowId>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_row_id",
        })
    }

    /// 按标签读取 JDBC `RowId` 值。
    fn row_id_by_label(&self, _column_label: &str) -> Result<Option<JdbcRowId>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_row_id_by_label",
        })
    }

    /// 按 1-based 下标读取 JDBC `NClob` 资源。
    fn n_clob(&self, _column_index: usize) -> Result<Option<JdbcNClob>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_n_clob",
        })
    }

    /// 按标签读取 JDBC `NClob` 资源。
    fn n_clob_by_label(&self, _column_label: &str) -> Result<Option<JdbcNClob>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_n_clob_by_label",
        })
    }

    /// 按 1-based 下标读取 JDBC `SQLXML` 资源。
    fn sql_xml(&self, _column_index: usize) -> Result<Option<JdbcSqlXml>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_sql_xml",
        })
    }

    /// 按标签读取 JDBC `SQLXML` 资源。
    fn sql_xml_by_label(&self, _column_label: &str) -> Result<Option<JdbcSqlXml>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_sql_xml_by_label",
        })
    }

    /// 按 1-based 下标执行标量或流更新。
    ///
    /// `update` 原样保留 Java setter 的类型和长度重载身份。
    fn update_value(
        &self,
        _column_index: usize,
        _update: &ResultSetUpdate,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value",
        })
    }

    /// 按列标签执行标量或流更新。
    fn update_value_by_label(
        &self,
        _column_label: &str,
        _update: &ResultSetUpdate,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value_by_label",
        })
    }

    /// 按下标更新 JDBC `Ref`；`None` 对应 Java null。
    fn update_reference(
        &self,
        _column_index: usize,
        _value: Option<&JdbcRef>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_ref",
        })
    }

    /// 按标签更新 JDBC `Ref`。
    fn update_reference_by_label(
        &self,
        _column_label: &str,
        _value: Option<&JdbcRef>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_ref_by_label",
        })
    }

    /// 按下标更新 JDBC `Blob`。
    fn update_blob(
        &self,
        _column_index: usize,
        _value: Option<&JdbcBlob>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_blob",
        })
    }

    /// 按标签更新 JDBC `Blob`。
    fn update_blob_by_label(
        &self,
        _column_label: &str,
        _value: Option<&JdbcBlob>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_blob_by_label",
        })
    }

    /// 按下标更新 JDBC `Clob`。
    fn update_clob(
        &self,
        _column_index: usize,
        _value: Option<&JdbcClob>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_clob",
        })
    }

    /// 按标签更新 JDBC `Clob`。
    fn update_clob_by_label(
        &self,
        _column_label: &str,
        _value: Option<&JdbcClob>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_clob_by_label",
        })
    }

    /// 按下标更新 JDBC `Array`。
    fn update_array(
        &self,
        _column_index: usize,
        _value: Option<&JdbcArray>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_array",
        })
    }

    /// 按标签更新 JDBC `Array`。
    fn update_array_by_label(
        &self,
        _column_label: &str,
        _value: Option<&JdbcArray>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_array_by_label",
        })
    }

    /// 按下标更新 JDBC `RowId`。
    fn update_row_id(
        &self,
        _column_index: usize,
        _value: Option<&JdbcRowId>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_row_id",
        })
    }

    /// 按标签更新 JDBC `RowId`。
    fn update_row_id_by_label(
        &self,
        _column_label: &str,
        _value: Option<&JdbcRowId>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_row_id_by_label",
        })
    }

    /// 按下标更新 JDBC `NClob`。
    fn update_n_clob(
        &self,
        _column_index: usize,
        _value: Option<&JdbcNClob>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_n_clob",
        })
    }

    /// 按标签更新 JDBC `NClob`。
    fn update_n_clob_by_label(
        &self,
        _column_label: &str,
        _value: Option<&JdbcNClob>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_n_clob_by_label",
        })
    }

    /// 按下标更新 JDBC `SQLXML`。
    fn update_sql_xml(
        &self,
        _column_index: usize,
        _value: Option<&JdbcSqlXml>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_sql_xml",
        })
    }

    /// 按标签更新 JDBC `SQLXML`。
    fn update_sql_xml_by_label(
        &self,
        _column_label: &str,
        _value: Option<&JdbcSqlXml>,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_sql_xml_by_label",
        })
    }

    /// 按下标使用输入流更新 `Blob`。
    fn update_blob_stream(
        &self,
        _column_index: usize,
        _stream: Option<&JdbcInputStream>,
        _length: JdbcStreamLength,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_blob_stream",
        })
    }

    /// 按标签使用输入流更新 `Blob`。
    fn update_blob_stream_by_label(
        &self,
        _column_label: &str,
        _stream: Option<&JdbcInputStream>,
        _length: JdbcStreamLength,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_blob_stream_by_label",
        })
    }

    /// 按下标使用 Reader 更新 `Clob`。
    fn update_clob_reader(
        &self,
        _column_index: usize,
        _reader: Option<&JdbcReader>,
        _length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_clob_reader",
        })
    }

    /// 按标签使用 Reader 更新 `Clob`。
    fn update_clob_reader_by_label(
        &self,
        _column_label: &str,
        _reader: Option<&JdbcReader>,
        _length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_clob_reader_by_label",
        })
    }

    /// 按下标使用 Reader 更新 `NClob`。
    fn update_n_clob_reader(
        &self,
        _column_index: usize,
        _reader: Option<&JdbcReader>,
        _length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_n_clob_reader",
        })
    }

    /// 按标签使用 Reader 更新 `NClob`。
    fn update_n_clob_reader_by_label(
        &self,
        _column_label: &str,
        _reader: Option<&JdbcReader>,
        _length: JdbcCharacterLength,
    ) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_n_clob_reader_by_label",
        })
    }

    /// 按列标签查找 1-based 下标。
    fn find_column(&self, _column_label: &str) -> Result<usize, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_find_column",
        })
    }

    /// 返回最近一次读取是否为 SQL NULL。
    fn was_null(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_was_null",
        })
    }

    /// 按 1-based 下标返回 ASCII 输入流。
    fn ascii_stream(&self, _column_index: usize) -> Result<Option<JdbcInputStream>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_ascii_stream",
        })
    }

    /// 按列标签返回 ASCII 输入流，保留 JDBC 标签重载身份。
    fn ascii_stream_by_label(
        &self,
        _column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_ascii_stream_by_label",
        })
    }

    /// 按 1-based 下标返回已废弃的 Unicode 输入流。
    fn unicode_stream(&self, _column_index: usize) -> Result<Option<JdbcInputStream>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_unicode_stream",
        })
    }

    /// 按列标签返回已废弃的 Unicode 输入流，保留 JDBC 标签重载身份。
    fn unicode_stream_by_label(
        &self,
        _column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_unicode_stream_by_label",
        })
    }

    /// 按 1-based 下标返回二进制输入流。
    fn binary_stream(&self, _column_index: usize) -> Result<Option<JdbcInputStream>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_binary_stream",
        })
    }

    /// 按列标签返回二进制输入流，保留 JDBC 标签重载身份。
    fn binary_stream_by_label(
        &self,
        _column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_binary_stream_by_label",
        })
    }

    /// 按 1-based 下标返回字符 Reader。
    fn character_stream(&self, _column_index: usize) -> Result<Option<JdbcReader>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_character_stream",
        })
    }

    /// 按列标签返回字符 Reader，保留 JDBC 标签重载身份。
    fn character_stream_by_label(
        &self,
        _column_label: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_character_stream_by_label",
        })
    }

    /// 按 1-based 下标返回国家字符集 Reader。
    fn n_character_stream(&self, _column_index: usize) -> Result<Option<JdbcReader>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_n_character_stream",
        })
    }

    /// 按列标签返回国家字符集 Reader，保留 JDBC 标签重载身份。
    fn n_character_stream_by_label(
        &self,
        _column_label: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_n_character_stream_by_label",
        })
    }

    /// 返回是否位于第一行之前。
    fn is_before_first(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_is_before_first",
        })
    }

    /// 返回是否位于最后一行之后。
    fn is_after_last(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_is_after_last",
        })
    }

    /// 返回是否位于第一行。
    fn is_first(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_is_first",
        })
    }

    /// 返回是否位于最后一行。
    fn is_last(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_is_last",
        })
    }

    /// 设置抓取方向。
    fn set_fetch_direction(&self, _direction: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_set_fetch_direction",
        })
    }

    /// 返回抓取方向。
    fn fetch_direction(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_fetch_direction",
        })
    }

    /// 设置抓取大小。
    fn set_fetch_size(&self, _rows: i32) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_set_fetch_size",
        })
    }

    /// 返回抓取大小。
    fn fetch_size(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_fetch_size",
        })
    }

    /// 返回结果集类型。
    fn result_set_type(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_type",
        })
    }

    /// 返回并发模式。
    fn concurrency(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_concurrency",
        })
    }

    /// 返回保持性。
    fn holdability(&self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_holdability",
        })
    }

    /// 返回警告链。
    fn warnings(&self) -> Result<Option<SqlWarning>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_warnings",
        })
    }

    /// 清除警告链。
    fn clear_warnings(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_clear_warnings",
        })
    }

    /// 返回可空游标名称。
    fn cursor_name(&self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_cursor_name",
        })
    }

    /// 返回结果列 metadata。
    fn meta_data(&self) -> Result<ResultSetMetaData, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_meta_data",
        })
    }

    /// 返回当前行是否被更新。
    fn row_updated(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_row_updated",
        })
    }

    /// 返回当前行是否为插入行。
    fn row_inserted(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_row_inserted",
        })
    }

    /// 返回当前行是否被删除。
    fn row_deleted(&self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_row_deleted",
        })
    }

    /// 提交插入行。
    fn insert_row(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_insert_row",
        })
    }

    /// 提交当前行更新。
    fn update_row(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_row",
        })
    }

    /// 删除当前行。
    fn delete_row(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_delete_row",
        })
    }

    /// 刷新当前行。
    fn refresh_row(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_refresh_row",
        })
    }

    /// 取消当前行尚未提交的更新。
    fn cancel_row_updates(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_cancel_row_updates",
        })
    }

    /// 移到插入行。
    fn move_to_insert_row(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_move_to_insert_row",
        })
    }

    /// 从插入行返回当前行。
    fn move_to_current_row(&self) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_move_to_current_row",
        })
    }
}

#[derive(Debug)]
struct RowSetResultSetState {
    closed: bool,
    cursor: i64,
    was_null: bool,
    fetch_direction: i32,
    fetch_size: i32,
    warnings: Option<SqlWarning>,
}

/// `Vec<Row>` 的完整可滚动只读结果集实现。
///
/// 这是当前 Toasty、SQLx 与 RBDC eager-fetch Adapter 的平台实现；它保留
/// JDBC 1-based 列下标、游标边界、NULL 状态及关闭错误，不把结果集退化成
/// 只能遍历一次的裸数组。
#[derive(Debug)]
pub struct RowSetResultSet {
    rows: Vec<Row>,
    column_labels: Vec<String>,
    meta_data: ResultSetMetaData,
    state: Mutex<RowSetResultSetState>,
}

impl RowSetResultSet {
    /// 从无列标签的行创建结果集。
    pub fn new(rows: Vec<Row>) -> Self {
        Self::with_column_labels(rows, Vec::new())
    }

    /// 从行和按列顺序排列的标签创建结果集。
    pub fn with_column_labels(rows: Vec<Row>, column_labels: Vec<String>) -> Self {
        let column_count = rows
            .iter()
            .map(Row::len)
            .max()
            .unwrap_or_default()
            .max(column_labels.len());
        let columns = (0..column_count)
            .map(|column_index| {
                let values = rows
                    .iter()
                    .filter_map(|row| row.get(column_index))
                    .collect::<Vec<_>>();
                let column_type = values
                    .iter()
                    .find_map(|value| match value {
                        Value::Null => None,
                        Value::Bool(_) => Some(ResultSetColumnType::Boolean),
                        Value::Int(_) => Some(ResultSetColumnType::Integer),
                        Value::Float(_) => Some(ResultSetColumnType::Float),
                        Value::Decimal(_) => Some(ResultSetColumnType::Decimal),
                        Value::Date(_) => Some(ResultSetColumnType::Date),
                        Value::Time(_) => Some(ResultSetColumnType::Time),
                        Value::Timestamp(_) => Some(ResultSetColumnType::Timestamp),
                        Value::String(_) => Some(ResultSetColumnType::Text),
                        Value::Bytes(_) => Some(ResultSetColumnType::Binary),
                    })
                    .unwrap_or(ResultSetColumnType::Unknown);
                let nullable = values.len() != rows.len()
                    || values.iter().any(|value| matches!(value, Value::Null));
                ResultSetColumnMeta::new(
                    column_labels.get(column_index).cloned().unwrap_or_default(),
                    column_type,
                    nullable,
                )
            })
            .collect();
        Self {
            rows,
            column_labels,
            meta_data: ResultSetMetaData::new(columns),
            state: Mutex::new(RowSetResultSetState {
                closed: false,
                cursor: -1,
                was_null: false,
                fetch_direction: 1000,
                fetch_size: 0,
                warnings: None,
            }),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, RowSetResultSetState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn ensure_open(state: &RowSetResultSetState) -> Result<(), DruidError> {
        if state.closed {
            Err(DruidError::Other("result set is closed".to_string()))
        } else {
            Ok(())
        }
    }

    fn len(&self) -> i64 {
        self.rows.len() as i64
    }

    fn has_current(&self, cursor: i64) -> bool {
        cursor >= 0 && cursor < self.len()
    }

    fn move_to(&self, state: &mut RowSetResultSetState, cursor: i64) -> bool {
        if self.has_current(cursor) {
            state.cursor = cursor;
            true
        } else {
            state.cursor = if cursor < 0 { -1 } else { self.len() };
            false
        }
    }
}

impl PhysicalResultSet for RowSetResultSet {
    fn close(&self) -> Result<(), DruidError> {
        self.state().closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.state().closed
    }

    fn next(&self) -> Result<bool, DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        let next = state.cursor.saturating_add(1);
        Ok(self.move_to(&mut state, next))
    }

    fn previous(&self) -> Result<bool, DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        let previous = if state.cursor >= self.len() {
            self.len().saturating_sub(1)
        } else {
            state.cursor.saturating_sub(1)
        };
        Ok(self.move_to(&mut state, previous))
    }

    fn first(&self) -> Result<bool, DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        Ok(self.move_to(&mut state, 0))
    }

    fn last(&self) -> Result<bool, DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        Ok(self.move_to(&mut state, self.len().saturating_sub(1)))
    }

    fn before_first(&self) -> Result<(), DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        state.cursor = -1;
        Ok(())
    }

    fn after_last(&self) -> Result<(), DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        state.cursor = self.len();
        Ok(())
    }

    fn absolute(&self, row: i32) -> Result<bool, DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        if row == 0 {
            state.cursor = -1;
            return Ok(false);
        }
        let cursor = if row > 0 {
            i64::from(row) - 1
        } else {
            self.len() + i64::from(row)
        };
        Ok(self.move_to(&mut state, cursor))
    }

    fn relative(&self, rows: i32) -> Result<bool, DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        let cursor = state.cursor.saturating_add(i64::from(rows));
        Ok(self.move_to(&mut state, cursor))
    }

    fn row(&self) -> Result<i32, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(if self.has_current(state.cursor) {
            (state.cursor + 1) as i32
        } else {
            0
        })
    }

    fn value(&self, column_index: usize) -> Result<Value, DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        if !self.has_current(state.cursor) {
            return Err(DruidError::Other(
                "result set cursor is not positioned on a row".to_string(),
            ));
        }
        let index = column_index
            .checked_sub(1)
            .ok_or_else(|| DruidError::InvalidArgument("column_index is 1-based".to_string()))?;
        let value = self.rows[state.cursor as usize]
            .get(index)
            .cloned()
            .ok_or_else(|| {
                DruidError::InvalidArgument(format!(
                    "column_index {column_index} exceeds row width"
                ))
            })?;
        state.was_null = matches!(value, Value::Null);
        Ok(value)
    }

    fn find_column(&self, column_label: &str) -> Result<usize, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        self.column_labels
            .iter()
            .position(|label| label.eq_ignore_ascii_case(column_label))
            .map(|index| index + 1)
            .ok_or_else(|| DruidError::InvalidArgument(format!("unknown column: {column_label}")))
    }

    fn was_null(&self) -> Result<bool, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.was_null)
    }

    fn ascii_stream(&self, column_index: usize) -> Result<Option<JdbcInputStream>, DruidError> {
        self.binary_stream(column_index)
    }

    fn ascii_stream_by_label(
        &self,
        column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.ascii_stream(self.find_column(column_label)?)
    }

    fn unicode_stream(&self, column_index: usize) -> Result<Option<JdbcInputStream>, DruidError> {
        self.binary_stream(column_index)
    }

    fn unicode_stream_by_label(
        &self,
        column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.unicode_stream(self.find_column(column_label)?)
    }

    fn binary_stream(&self, column_index: usize) -> Result<Option<JdbcInputStream>, DruidError> {
        match self.value(column_index)? {
            Value::Null => Ok(None),
            Value::Bytes(value) => Ok(Some(JdbcInputStream::from_bytes(value))),
            Value::String(value) => Ok(Some(JdbcInputStream::from_bytes(value.into_bytes()))),
            Value::Bool(value) => Ok(Some(JdbcInputStream::from_bytes(
                value.to_string().into_bytes(),
            ))),
            Value::Int(value) => Ok(Some(JdbcInputStream::from_bytes(
                value.to_string().into_bytes(),
            ))),
            Value::Float(value) => Ok(Some(JdbcInputStream::from_bytes(
                value.to_string().into_bytes(),
            ))),
            Value::Decimal(value) => Ok(Some(JdbcInputStream::from_bytes(
                value.to_string().into_bytes(),
            ))),
            Value::Date(value) => Ok(Some(JdbcInputStream::from_bytes(
                value.to_string().into_bytes(),
            ))),
            Value::Time(value) => Ok(Some(JdbcInputStream::from_bytes(
                value.to_string().into_bytes(),
            ))),
            Value::Timestamp(value) => Ok(Some(JdbcInputStream::from_bytes(
                value.to_string().into_bytes(),
            ))),
        }
    }

    fn binary_stream_by_label(
        &self,
        column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.binary_stream(self.find_column(column_label)?)
    }

    fn character_stream(&self, column_index: usize) -> Result<Option<JdbcReader>, DruidError> {
        match self.value(column_index)? {
            Value::Null => Ok(None),
            Value::String(value) => Ok(Some(JdbcReader::from_string(value))),
            Value::Bytes(value) => String::from_utf8(value)
                .map(JdbcReader::from_string)
                .map(Some)
                .map_err(|error| DruidError::DriverError(error.to_string())),
            Value::Bool(value) => Ok(Some(JdbcReader::from_string(value.to_string()))),
            Value::Int(value) => Ok(Some(JdbcReader::from_string(value.to_string()))),
            Value::Float(value) => Ok(Some(JdbcReader::from_string(value.to_string()))),
            Value::Decimal(value) => Ok(Some(JdbcReader::from_string(value.to_string()))),
            Value::Date(value) => Ok(Some(JdbcReader::from_string(value.to_string()))),
            Value::Time(value) => Ok(Some(JdbcReader::from_string(value.to_string()))),
            Value::Timestamp(value) => Ok(Some(JdbcReader::from_string(value.to_string()))),
        }
    }

    fn character_stream_by_label(
        &self,
        column_label: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        self.character_stream(self.find_column(column_label)?)
    }

    fn n_character_stream(&self, column_index: usize) -> Result<Option<JdbcReader>, DruidError> {
        self.character_stream(column_index)
    }

    fn n_character_stream_by_label(
        &self,
        column_label: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        self.n_character_stream(self.find_column(column_label)?)
    }

    fn is_before_first(&self) -> Result<bool, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(!self.rows.is_empty() && state.cursor < 0)
    }

    fn is_after_last(&self) -> Result<bool, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(!self.rows.is_empty() && state.cursor >= self.len())
    }

    fn is_first(&self) -> Result<bool, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(!self.rows.is_empty() && state.cursor == 0)
    }

    fn is_last(&self) -> Result<bool, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(!self.rows.is_empty() && state.cursor == self.len() - 1)
    }

    fn set_fetch_direction(&self, direction: i32) -> Result<(), DruidError> {
        if ![1000, 1001, 1002].contains(&direction) {
            return Err(DruidError::InvalidArgument(format!(
                "invalid fetch direction: {direction}"
            )));
        }
        let mut state = self.state();
        Self::ensure_open(&state)?;
        state.fetch_direction = direction;
        Ok(())
    }

    fn fetch_direction(&self) -> Result<i32, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.fetch_direction)
    }

    fn set_fetch_size(&self, rows: i32) -> Result<(), DruidError> {
        if rows < 0 {
            return Err(DruidError::InvalidArgument(
                "fetch_size must not be negative".to_string(),
            ));
        }
        let mut state = self.state();
        Self::ensure_open(&state)?;
        state.fetch_size = rows;
        Ok(())
    }

    fn fetch_size(&self) -> Result<i32, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.fetch_size)
    }

    fn result_set_type(&self) -> Result<i32, DruidError> {
        Ok(1004)
    }

    fn concurrency(&self) -> Result<i32, DruidError> {
        Ok(1007)
    }

    fn holdability(&self) -> Result<i32, DruidError> {
        Ok(1)
    }

    fn warnings(&self) -> Result<Option<SqlWarning>, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(state.warnings.clone())
    }

    fn clear_warnings(&self) -> Result<(), DruidError> {
        let mut state = self.state();
        Self::ensure_open(&state)?;
        state.warnings = None;
        Ok(())
    }

    fn cursor_name(&self) -> Result<Option<String>, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(None)
    }

    fn meta_data(&self) -> Result<ResultSetMetaData, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(self.meta_data.clone())
    }

    fn row_updated(&self) -> Result<bool, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(false)
    }

    fn row_inserted(&self) -> Result<bool, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(false)
    }

    fn row_deleted(&self) -> Result<bool, DruidError> {
        let state = self.state();
        Self::ensure_open(&state)?;
        Ok(false)
    }
}

fn value_to_jdbc_object(
    value: Value,
    target_type: &JdbcTargetType,
) -> Result<JdbcObject, DruidError> {
    if matches!(value, Value::Null) {
        return Ok(JdbcObject::Scalar(Value::Null));
    }

    match target_type {
        JdbcTargetType::String => value_to_string(value).map(JdbcObject::String),
        JdbcTargetType::Boolean => value_to_boolean(value).map(JdbcObject::Boolean),
        JdbcTargetType::Byte => i8::try_from(value_to_long(value)?)
            .map(JdbcObject::Byte)
            .map_err(|error| DruidError::DriverError(error.to_string())),
        JdbcTargetType::Short => i16::try_from(value_to_long(value)?)
            .map(JdbcObject::Short)
            .map_err(|error| DruidError::DriverError(error.to_string())),
        JdbcTargetType::Integer => i32::try_from(value_to_long(value)?)
            .map(JdbcObject::Integer)
            .map_err(|error| DruidError::DriverError(error.to_string())),
        JdbcTargetType::Long => value_to_long(value).map(JdbcObject::Long),
        JdbcTargetType::Float => {
            value_to_double(value).map(|value| JdbcObject::Float(value as f32))
        }
        JdbcTargetType::Double => value_to_double(value).map(JdbcObject::Double),
        JdbcTargetType::Bytes => value_to_bytes(value).map(JdbcObject::Bytes),
        JdbcTargetType::BigDecimal => value_to_big_decimal(value, None)?
            .map(JdbcObject::BigDecimal)
            .ok_or_else(|| DruidError::DriverError("unexpected SQL NULL".to_string())),
        JdbcTargetType::Date => value_to_date(value)?
            .map(JdbcObject::Date)
            .ok_or_else(|| DruidError::DriverError("unexpected SQL NULL".to_string())),
        JdbcTargetType::Time => value_to_time(value)?
            .map(JdbcObject::Time)
            .ok_or_else(|| DruidError::DriverError("unexpected SQL NULL".to_string())),
        JdbcTargetType::Timestamp => value_to_timestamp(value)?
            .map(JdbcObject::Timestamp)
            .ok_or_else(|| DruidError::DriverError("unexpected SQL NULL".to_string())),
        JdbcTargetType::Blob
        | JdbcTargetType::Clob
        | JdbcTargetType::NClob
        | JdbcTargetType::Array
        | JdbcTargetType::Ref
        | JdbcTargetType::RowId
        | JdbcTargetType::SqlXml
        | JdbcTargetType::Url
        | JdbcTargetType::Custom(_) => Err(DruidError::UnsupportedOperation {
            operation: "result_set_get_object_typed_resource",
        }),
    }
}

fn value_to_string(value: Value) -> Result<String, DruidError> {
    match value {
        Value::String(value) => Ok(value),
        Value::Bytes(value) => {
            String::from_utf8(value).map_err(|error| DruidError::DriverError(error.to_string()))
        }
        Value::Bool(value) => Ok(value.to_string()),
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(value.to_string()),
        Value::Decimal(value) => Ok(value.to_string()),
        Value::Date(value) => Ok(value.to_string()),
        Value::Time(value) => Ok(value.to_string()),
        Value::Timestamp(value) => Ok(value.to_string()),
        Value::Null => Err(DruidError::DriverError("unexpected SQL NULL".to_string())),
    }
}

fn value_to_boolean(value: Value) -> Result<bool, DruidError> {
    match value {
        Value::Bool(value) => Ok(value),
        Value::Int(value) => Ok(value != 0),
        Value::Float(value) => Ok(value != 0.0),
        Value::Decimal(value) => Ok(value != BigDecimal::from(0)),
        Value::String(value) => Ok(value == "1" || value.eq_ignore_ascii_case("true")),
        actual => Err(result_set_type_error("Boolean", &actual)),
    }
}

fn value_to_long(value: Value) -> Result<i64, DruidError> {
    match value {
        Value::Bool(value) => Ok(i64::from(value)),
        Value::Int(value) => Ok(value),
        Value::Float(value) => Ok(value as i64),
        Value::Decimal(value) => value
            .to_i64()
            .ok_or_else(|| result_set_type_error("Long", &Value::Decimal(value))),
        Value::String(value) => value
            .parse::<i64>()
            .map_err(|error| DruidError::DriverError(error.to_string())),
        actual => Err(result_set_type_error("Long", &actual)),
    }
}

fn value_to_double(value: Value) -> Result<f64, DruidError> {
    match value {
        Value::Bool(value) => Ok(if value { 1.0 } else { 0.0 }),
        Value::Int(value) => Ok(value as f64),
        Value::Float(value) => Ok(value),
        Value::Decimal(value) => value
            .to_f64()
            .ok_or_else(|| result_set_type_error("Double", &Value::Decimal(value))),
        Value::String(value) => value
            .parse::<f64>()
            .map_err(|error| DruidError::DriverError(error.to_string())),
        actual => Err(result_set_type_error("Double", &actual)),
    }
}

fn value_to_bytes(value: Value) -> Result<Vec<u8>, DruidError> {
    match value {
        Value::Bytes(value) => Ok(value),
        other => value_to_string(other).map(String::into_bytes),
    }
}

fn value_to_big_decimal(
    value: Value,
    scale: Option<i32>,
) -> Result<Option<BigDecimal>, DruidError> {
    let value = match value {
        Value::Null => return Ok(None),
        Value::Decimal(value) => value,
        Value::Int(value) => BigDecimal::from(value),
        Value::Float(value) => BigDecimal::from_f64(value).ok_or_else(|| {
            DruidError::DriverError(format!(
                "ResultSet floating point value {value} cannot be represented as BigDecimal"
            ))
        })?,
        Value::String(value) => BigDecimal::from_str(&value).map_err(|error| {
            DruidError::DriverError(format!(
                "ResultSet value {value:?} cannot be converted to BigDecimal: {error}"
            ))
        })?,
        actual => return Err(result_set_type_error("BigDecimal", &actual)),
    };
    Ok(Some(match scale {
        Some(scale) => value.with_scale(i64::from(scale)),
        None => value,
    }))
}

fn value_to_date(value: Value) -> Result<Option<NaiveDate>, DruidError> {
    match value {
        Value::Null => Ok(None),
        Value::Date(value) => Ok(Some(value)),
        Value::Timestamp(value) => Ok(Some(value.date())),
        Value::String(value) => NaiveDate::parse_from_str(&value, "%Y-%m-%d")
            .map(Some)
            .map_err(|error| {
                DruidError::DriverError(format!(
                    "ResultSet value {value:?} cannot be converted to Date: {error}"
                ))
            }),
        actual => Err(result_set_type_error("Date", &actual)),
    }
}

fn value_to_time(value: Value) -> Result<Option<NaiveTime>, DruidError> {
    match value {
        Value::Null => Ok(None),
        Value::Time(value) => Ok(Some(value)),
        Value::Timestamp(value) => Ok(Some(value.time())),
        Value::String(value) => NaiveTime::parse_from_str(&value, "%H:%M:%S%.f")
            .map(Some)
            .map_err(|error| {
                DruidError::DriverError(format!(
                    "ResultSet value {value:?} cannot be converted to Time: {error}"
                ))
            }),
        actual => Err(result_set_type_error("Time", &actual)),
    }
}

fn value_to_timestamp(value: Value) -> Result<Option<NaiveDateTime>, DruidError> {
    match value {
        Value::Null => Ok(None),
        Value::Timestamp(value) => Ok(Some(value)),
        // 有效 `NaiveDate` 与午夜组合不存在失败分支。
        Value::Date(value) => Ok(Some(NaiveDateTime::new(value, NaiveTime::MIN))),
        Value::String(value) => parse_timestamp(&value).map(Some).map_err(|error| {
            DruidError::DriverError(format!(
                "ResultSet value {value:?} cannot be converted to Timestamp: {error}"
            ))
        }),
        actual => Err(result_set_type_error("Timestamp", &actual)),
    }
}

fn parse_timestamp(value: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f"))
}

fn result_set_type_error(expected: &str, actual: &Value) -> DruidError {
    DruidError::DriverError(format!("ResultSet expected {expected} value, got {actual}"))
}

/// 不泄漏具体驱动类型的结果集句柄。
#[derive(Clone)]
pub struct JdbcResultSet {
    physical: Arc<dyn PhysicalResultSet>,
}

impl JdbcResultSet {
    /// 包装物理结果集。
    pub fn new(physical: Arc<dyn PhysicalResultSet>) -> Self {
        Self { physical }
    }

    /// 关闭结果集。
    pub fn close(&self) -> Result<(), DruidError> {
        self.physical.close()
    }

    /// 返回是否关闭。
    pub fn is_closed(&self) -> bool {
        self.physical.is_closed()
    }

    /// 返回物理结果集 SPI。
    pub fn physical(&self) -> &dyn PhysicalResultSet {
        self.physical.as_ref()
    }
}

impl fmt::Debug for JdbcResultSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcResultSet")
            .field("physical", &self.physical)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for JdbcResultSet {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcResultSet {}

#[cfg(test)]
mod tests {
    use super::{value_to_jdbc_object, value_to_string};
    use crate::core::{DruidError, JdbcTargetType, Value};

    #[test]
    fn internal_scalar_converter_rejects_resource_targets_explicitly() {
        assert!(matches!(
            value_to_jdbc_object(Value::Int(1), &JdbcTargetType::Blob),
            Err(DruidError::UnsupportedOperation {
                operation: "result_set_get_object_typed_resource"
            })
        ));
        assert!(matches!(
            value_to_jdbc_object(
                Value::Int(1),
                &JdbcTargetType::Custom("vendor.Type".to_string())
            ),
            Err(DruidError::UnsupportedOperation {
                operation: "result_set_get_object_typed_resource"
            })
        ));
    }

    #[test]
    fn internal_non_null_string_converter_rejects_accidental_null_input() {
        assert!(matches!(
            value_to_string(Value::Null),
            Err(DruidError::DriverError(message)) if message == "unexpected SQL NULL"
        ));
    }
}
