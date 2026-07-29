//! `ResultSet` 同步 `Filter` 协议。
//!
//! 对应 Java：`com.alibaba.druid.filter.Filter` 的 `resultSet_*` 方法族。

use super::{
    DruidError, JdbcArray, JdbcBlob, JdbcCalendarArgument, JdbcClob, JdbcInputStream, JdbcNClob,
    JdbcObject, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcTargetType, JdbcTypeMap, JdbcUrl,
    ResultSetFilterChain, ResultSetFilterContext, ResultSetMetaData, ResultSetStatement,
    SqlWarning, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

macro_rules! scalar_getter_filter_methods {
    ($(($index:ident, $label:ident, $chain_index:ident, $chain_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "(int)`。")]
            fn $index(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
            ) -> Result<$ty, DruidError> {
                chain.$chain_index(column_index)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String)`。")]
            fn $label(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
            ) -> Result<$ty, DruidError> {
                chain.$chain_label(column_label)
            }
        )+
    };
}

macro_rules! temporal_getter_filter_methods {
    ($(($index:ident, $label:ident, $index_calendar:ident, $label_calendar:ident, $chain_index:ident, $chain_label:ident, $chain_index_calendar:ident, $chain_label_calendar:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "(int)`。")]
            fn $index(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
            ) -> Result<Option<$ty>, DruidError> {
                chain.$chain_index(column_index)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String)`。")]
            fn $label(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
            ) -> Result<Option<$ty>, DruidError> {
                chain.$chain_label(column_label)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(int, Calendar)`，保留显式 null Calendar。")]
            fn $index_calendar(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
                calendar: &JdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                chain.$chain_index_calendar(column_index, calendar)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String, Calendar)`，保留显式 null Calendar。")]
            fn $label_calendar(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
                calendar: &JdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                chain.$chain_label_calendar(column_label, calendar)
            }
        )+
    };
}

macro_rules! resource_getter_filter_methods {
    ($(($index:ident, $label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "(int)`，保留资源句柄。")]
            fn $index(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
            ) -> Result<Option<$ty>, DruidError> {
                chain.$index(column_index)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String)`，保持标签重载身份。")]
            fn $label(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
            ) -> Result<Option<$ty>, DruidError> {
                chain.$label(column_label)
            }
        )+
    };
}

macro_rules! no_arg_result_set_filter_methods {
    ($(($method:ident, $chain_method:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "()`。")]
            fn $method(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
            ) -> Result<$ty, DruidError> {
                chain.$chain_method()
            }
        )+
    };
}

macro_rules! i32_arg_result_set_filter_methods {
    ($(($method:ident, $chain_method:ident, $argument:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "(int)`，保留参数身份。")]
            fn $method(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                $argument: i32,
            ) -> Result<$ty, DruidError> {
                chain.$chain_method($argument)
            }
        )+
    };
}

macro_rules! scalar_update_filter_methods {
    ($(($index:ident, $label:ident, $chain_index:ident, $chain_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "(int, ..)`，保留 setter 类型身份。")]
            fn $index(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
                value: $ty,
            ) -> Result<(), DruidError> {
                chain.$chain_index(column_index, value)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String, ..)`，保留标签重载身份。")]
            fn $label(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
                value: $ty,
            ) -> Result<(), DruidError> {
                chain.$chain_label(column_label, value)
            }
        )+
    };
}

macro_rules! resource_update_filter_methods {
    ($(($index:ident, $label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "(int, ..)`，保留 nullable 资源句柄身份。")]
            fn $index(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                chain.$index(column_index, value)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String, ..)`，保留标签与 nullable 资源句柄身份。")]
            fn $label(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                chain.$label(column_label, value)
            }
        )+
    };
}

macro_rules! lob_stream_update_filter_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $ty:ty,
        $java:literal
    )),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "(int, stream/reader)`，保留无长度重载身份。")]
            fn $index(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                chain.$index(column_index, value)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String, stream/reader)`，保留标签与无长度重载身份。")]
            fn $label(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                chain.$label(column_label, value)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(int, stream/reader, long)`，长度原样下沉。")]
            fn $index_with_length(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                chain.$index_with_length(column_index, value, length)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String, stream/reader, long)`，保留标签与长度身份。")]
            fn $label_with_length(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                chain.$label_with_length(column_label, value, length)
            }
        )+
    };
}

macro_rules! stream_update_filter_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_int_length:ident,
        $label_with_int_length:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $ty:ty,
        $java:literal
    )),+ $(,)?) => {
        $(
            #[doc = concat!("包围 Java `ResultSet#", $java, "(int, stream/reader)`，保留无长度重载身份。")]
            fn $index(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                chain.$index(column_index, value)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String, stream/reader)`，保留标签与无长度重载身份。")]
            fn $label(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
                value: Option<$ty>,
            ) -> Result<(), DruidError> {
                chain.$label(column_label, value)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(int, stream/reader, int)`，保留 int 长度重载身份。")]
            fn $index_with_int_length(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
                value: Option<$ty>,
                length: i32,
            ) -> Result<(), DruidError> {
                chain.$index_with_int_length(column_index, value, length)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String, stream/reader, int)`，保留标签与 int 长度身份。")]
            fn $label_with_int_length(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
                value: Option<$ty>,
                length: i32,
            ) -> Result<(), DruidError> {
                chain.$label_with_int_length(column_label, value, length)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(int, stream/reader, long)`，保留 long 长度重载身份。")]
            fn $index_with_length(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_index: usize,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                chain.$index_with_length(column_index, value, length)
            }

            #[doc = concat!("包围 Java `ResultSet#", $java, "(String, stream/reader, long)`，保留标签与 long 长度身份。")]
            fn $label_with_length(
                &self,
                chain: &mut ResultSetFilterChain<'_>,
                column_label: &str,
                value: Option<$ty>,
                length: i64,
            ) -> Result<(), DruidError> {
                chain.$label_with_length(column_label, value, length)
            }
        )+
    };
}

macro_rules! long_stream_update_filter_methods {
    ($((
        $index:ident,
        $label:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $ty:ty,
        $java:literal
    )),+ $(,)?) => {
        $(
            lob_stream_update_filter_methods!((
                $index,
                $label,
                $index_with_length,
                $label_with_length,
                $ty,
                $java
            ));
        )+
    };
}

/// 可包围 `ResultSet` 物理操作的同步 `Filter`。
///
/// Java JDBC `ResultSet` 操作是同步调用，`Filter` 可以在委托前后执行逻辑，也可以
/// 短路或改写返回值。因此该协议保留 around-chain，而不是退化为只读事件通知。
pub trait ResultSetFilter: Send + Sync {
    /// 查询成功并创建 `ResultSet` 代理后执行。
    ///
    /// 对应 Java：`FilterEventAdapter#resultSetOpenAfter(ResultSetProxy)`。
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        Ok(())
    }

    /// 包围 `ResultSet#next()`。
    ///
    /// 默认实现继续调用链；实现可以短路或改写返回值，与 Java Filter 一致。
    fn result_set_next(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<bool, DruidError> {
        chain.result_set_next()
    }

    /// 包围 `ResultSet#close()`。
    ///
    /// 默认实现继续调用链。委托前产生的副作用即使下游关闭失败也不会回滚。
    fn result_set_close(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<(), DruidError> {
        chain.result_set_close()
    }

    /// 包围 `ResultSet#getWarnings()`。
    ///
    /// 默认实现继续调用链；实现可以短路或改写警告链，与 Java Filter 一致。
    fn result_set_get_warnings(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        chain.result_set_get_warnings()
    }

    /// 包围 `ResultSet#clearWarnings()`。
    ///
    /// 默认实现继续调用链；下游未被调用时物理警告保持不变。
    fn result_set_clear_warnings(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        chain.result_set_clear_warnings()
    }

    /// 包围 Java `ResultSet#getObject(int)`。
    fn result_set_get_object(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        chain.result_set_get_object(column_index)
    }

    /// 包围 Java `ResultSet#getObject(String)`，保持标签重载身份。
    fn result_set_get_object_by_label(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        chain.result_set_get_object_by_label(column_label)
    }

    /// 包围 Java `ResultSet#getObject(int, Map<String, Class<?>>)`。
    ///
    /// `type_map=None` 精确保留 Java 显式传入 `null` Map 的调用。
    fn result_set_get_object_with_type_map(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        chain.result_set_get_object_with_type_map(column_index, type_map)
    }

    /// 包围 Java `ResultSet#getObject(String, Map<String, Class<?>>)`。
    fn result_set_get_object_by_label_with_type_map(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        chain.result_set_get_object_by_label_with_type_map(column_label, type_map)
    }

    /// 包围 Java `ResultSet#getObject(int, Class<T>)`。
    fn result_set_get_object_typed(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        chain.result_set_get_object_typed(column_index, target_type)
    }

    /// 包围 Java `ResultSet#getObject(String, Class<T>)`。
    fn result_set_get_object_typed_by_label(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        chain.result_set_get_object_typed_by_label(column_label, target_type)
    }

    scalar_getter_filter_methods!(
        (
            result_set_get_string,
            result_set_get_string_by_label,
            result_set_get_string,
            result_set_get_string_by_label,
            Option<String>,
            "getString"
        ),
        (
            result_set_get_boolean,
            result_set_get_boolean_by_label,
            result_set_get_boolean,
            result_set_get_boolean_by_label,
            bool,
            "getBoolean"
        ),
        (
            result_set_get_byte,
            result_set_get_byte_by_label,
            result_set_get_byte,
            result_set_get_byte_by_label,
            i8,
            "getByte"
        ),
        (
            result_set_get_short,
            result_set_get_short_by_label,
            result_set_get_short,
            result_set_get_short_by_label,
            i16,
            "getShort"
        ),
        (
            result_set_get_int,
            result_set_get_int_by_label,
            result_set_get_int,
            result_set_get_int_by_label,
            i32,
            "getInt"
        ),
        (
            result_set_get_long,
            result_set_get_long_by_label,
            result_set_get_long,
            result_set_get_long_by_label,
            i64,
            "getLong"
        ),
        (
            result_set_get_float,
            result_set_get_float_by_label,
            result_set_get_float,
            result_set_get_float_by_label,
            f32,
            "getFloat"
        ),
        (
            result_set_get_double,
            result_set_get_double_by_label,
            result_set_get_double,
            result_set_get_double_by_label,
            f64,
            "getDouble"
        ),
        (
            result_set_get_bytes,
            result_set_get_bytes_by_label,
            result_set_get_bytes,
            result_set_get_bytes_by_label,
            Option<Vec<u8>>,
            "getBytes"
        ),
        (
            result_set_get_n_string,
            result_set_get_n_string_by_label,
            result_set_get_n_string,
            result_set_get_n_string_by_label,
            Option<String>,
            "getNString"
        ),
    );

    /// 包围 Java `ResultSet#getBigDecimal(int)`。
    fn result_set_get_big_decimal(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        chain.result_set_get_big_decimal(column_index)
    }

    /// 包围 Java `ResultSet#getBigDecimal(String)`。
    fn result_set_get_big_decimal_by_label(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        chain.result_set_get_big_decimal_by_label(column_label)
    }

    /// 包围已废弃的 Java `ResultSet#getBigDecimal(int, int)`。
    fn result_set_get_big_decimal_with_scale(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        chain.result_set_get_big_decimal_with_scale(column_index, scale)
    }

    /// 包围已废弃的 Java `ResultSet#getBigDecimal(String, int)`。
    fn result_set_get_big_decimal_by_label_with_scale(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        chain.result_set_get_big_decimal_by_label_with_scale(column_label, scale)
    }

    temporal_getter_filter_methods!(
        (
            result_set_get_date,
            result_set_get_date_by_label,
            result_set_get_date_with_calendar,
            result_set_get_date_by_label_with_calendar,
            result_set_get_date,
            result_set_get_date_by_label,
            result_set_get_date_with_calendar,
            result_set_get_date_by_label_with_calendar,
            NaiveDate,
            "getDate"
        ),
        (
            result_set_get_time,
            result_set_get_time_by_label,
            result_set_get_time_with_calendar,
            result_set_get_time_by_label_with_calendar,
            result_set_get_time,
            result_set_get_time_by_label,
            result_set_get_time_with_calendar,
            result_set_get_time_by_label_with_calendar,
            NaiveTime,
            "getTime"
        ),
        (
            result_set_get_timestamp,
            result_set_get_timestamp_by_label,
            result_set_get_timestamp_with_calendar,
            result_set_get_timestamp_by_label_with_calendar,
            result_set_get_timestamp,
            result_set_get_timestamp_by_label,
            result_set_get_timestamp_with_calendar,
            result_set_get_timestamp_by_label_with_calendar,
            NaiveDateTime,
            "getTimestamp"
        ),
    );

    resource_getter_filter_methods!(
        (
            result_set_get_ref,
            result_set_get_ref_by_label,
            JdbcRef,
            "getRef"
        ),
        (
            result_set_get_blob,
            result_set_get_blob_by_label,
            JdbcBlob,
            "getBlob"
        ),
        (
            result_set_get_clob,
            result_set_get_clob_by_label,
            JdbcClob,
            "getClob"
        ),
        (
            result_set_get_array,
            result_set_get_array_by_label,
            JdbcArray,
            "getArray"
        ),
        (
            result_set_get_url,
            result_set_get_url_by_label,
            JdbcUrl,
            "getURL"
        ),
        (
            result_set_get_row_id,
            result_set_get_row_id_by_label,
            JdbcRowId,
            "getRowId"
        ),
        (
            result_set_get_n_clob,
            result_set_get_n_clob_by_label,
            JdbcNClob,
            "getNClob"
        ),
        (
            result_set_get_sql_xml,
            result_set_get_sql_xml_by_label,
            JdbcSqlXml,
            "getSQLXML"
        ),
        (
            result_set_get_ascii_stream,
            result_set_get_ascii_stream_by_label,
            JdbcInputStream,
            "getAsciiStream"
        ),
        (
            result_set_get_unicode_stream,
            result_set_get_unicode_stream_by_label,
            JdbcInputStream,
            "getUnicodeStream"
        ),
        (
            result_set_get_binary_stream,
            result_set_get_binary_stream_by_label,
            JdbcInputStream,
            "getBinaryStream"
        ),
        (
            result_set_get_character_stream,
            result_set_get_character_stream_by_label,
            JdbcReader,
            "getCharacterStream"
        ),
        (
            result_set_get_n_character_stream,
            result_set_get_n_character_stream_by_label,
            JdbcReader,
            "getNCharacterStream"
        ),
    );

    /// 包围 Java `ResultSet#updateNull(int)`。
    fn result_set_update_null(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<(), DruidError> {
        chain.result_set_update_null(column_index)
    }

    /// 包围 Java `ResultSet#updateNull(String)`。
    fn result_set_update_null_by_label(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<(), DruidError> {
        chain.result_set_update_null_by_label(column_label)
    }

    scalar_update_filter_methods!(
        (result_set_update_boolean, result_set_update_boolean_by_label, result_set_update_boolean, result_set_update_boolean_by_label, bool, "updateBoolean"),
        (result_set_update_byte, result_set_update_byte_by_label, result_set_update_byte, result_set_update_byte_by_label, i8, "updateByte"),
        (result_set_update_short, result_set_update_short_by_label, result_set_update_short, result_set_update_short_by_label, i16, "updateShort"),
        (result_set_update_int, result_set_update_int_by_label, result_set_update_int, result_set_update_int_by_label, i32, "updateInt"),
        (result_set_update_long, result_set_update_long_by_label, result_set_update_long, result_set_update_long_by_label, i64, "updateLong"),
        (result_set_update_float, result_set_update_float_by_label, result_set_update_float, result_set_update_float_by_label, f32, "updateFloat"),
        (result_set_update_double, result_set_update_double_by_label, result_set_update_double, result_set_update_double_by_label, f64, "updateDouble"),
        (result_set_update_big_decimal, result_set_update_big_decimal_by_label, result_set_update_big_decimal, result_set_update_big_decimal_by_label, Option<BigDecimal>, "updateBigDecimal"),
        (result_set_update_string, result_set_update_string_by_label, result_set_update_string, result_set_update_string_by_label, Option<String>, "updateString"),
        (result_set_update_n_string, result_set_update_n_string_by_label, result_set_update_n_string, result_set_update_n_string_by_label, Option<String>, "updateNString"),
        (result_set_update_bytes, result_set_update_bytes_by_label, result_set_update_bytes, result_set_update_bytes_by_label, Option<Vec<u8>>, "updateBytes"),
        (result_set_update_date, result_set_update_date_by_label, result_set_update_date, result_set_update_date_by_label, Option<NaiveDate>, "updateDate"),
        (result_set_update_time, result_set_update_time_by_label, result_set_update_time, result_set_update_time_by_label, Option<NaiveTime>, "updateTime"),
        (result_set_update_timestamp, result_set_update_timestamp_by_label, result_set_update_timestamp, result_set_update_timestamp_by_label, Option<NaiveDateTime>, "updateTimestamp"),
    );

    /// 包围 Java `ResultSet#updateObject(int, Object)`，保留平台对象身份。
    fn result_set_update_object(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        value: JdbcObject,
    ) -> Result<(), DruidError> {
        chain.result_set_update_object(column_index, value)
    }

    /// 包围 Java `ResultSet#updateObject(String, Object)`。
    fn result_set_update_object_by_label(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        value: JdbcObject,
    ) -> Result<(), DruidError> {
        chain.result_set_update_object_by_label(column_label, value)
    }

    /// 包围 Java `ResultSet#updateObject(int, Object, int)`。
    fn result_set_update_object_with_scale_or_length(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        value: JdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        chain.result_set_update_object_with_scale_or_length(column_index, value, scale_or_length)
    }

    /// 包围 Java `ResultSet#updateObject(String, Object, int)`。
    fn result_set_update_object_by_label_with_scale_or_length(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        value: JdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        chain.result_set_update_object_by_label_with_scale_or_length(
            column_label,
            value,
            scale_or_length,
        )
    }

    resource_update_filter_methods!(
        (
            result_set_update_reference,
            result_set_update_reference_by_label,
            JdbcRef,
            "updateRef"
        ),
        (
            result_set_update_blob,
            result_set_update_blob_by_label,
            JdbcBlob,
            "updateBlob"
        ),
        (
            result_set_update_clob,
            result_set_update_clob_by_label,
            JdbcClob,
            "updateClob"
        ),
        (
            result_set_update_array,
            result_set_update_array_by_label,
            JdbcArray,
            "updateArray"
        ),
        (
            result_set_update_row_id,
            result_set_update_row_id_by_label,
            JdbcRowId,
            "updateRowId"
        ),
        (
            result_set_update_n_clob,
            result_set_update_n_clob_by_label,
            JdbcNClob,
            "updateNClob"
        ),
        (
            result_set_update_sql_xml,
            result_set_update_sql_xml_by_label,
            JdbcSqlXml,
            "updateSQLXML"
        ),
    );

    lob_stream_update_filter_methods!(
        (
            result_set_update_blob_stream,
            result_set_update_blob_stream_by_label,
            result_set_update_blob_stream_with_length,
            result_set_update_blob_stream_by_label_with_length,
            JdbcInputStream,
            "updateBlob"
        ),
        (
            result_set_update_clob_reader,
            result_set_update_clob_reader_by_label,
            result_set_update_clob_reader_with_length,
            result_set_update_clob_reader_by_label_with_length,
            JdbcReader,
            "updateClob"
        ),
        (
            result_set_update_n_clob_reader,
            result_set_update_n_clob_reader_by_label,
            result_set_update_n_clob_reader_with_length,
            result_set_update_n_clob_reader_by_label_with_length,
            JdbcReader,
            "updateNClob"
        ),
    );

    stream_update_filter_methods!(
        (
            result_set_update_ascii_stream,
            result_set_update_ascii_stream_by_label,
            result_set_update_ascii_stream_with_int_length,
            result_set_update_ascii_stream_by_label_with_int_length,
            result_set_update_ascii_stream_with_length,
            result_set_update_ascii_stream_by_label_with_length,
            JdbcInputStream,
            "updateAsciiStream"
        ),
        (
            result_set_update_binary_stream,
            result_set_update_binary_stream_by_label,
            result_set_update_binary_stream_with_int_length,
            result_set_update_binary_stream_by_label_with_int_length,
            result_set_update_binary_stream_with_length,
            result_set_update_binary_stream_by_label_with_length,
            JdbcInputStream,
            "updateBinaryStream"
        ),
        (
            result_set_update_character_stream,
            result_set_update_character_stream_by_label,
            result_set_update_character_stream_with_int_length,
            result_set_update_character_stream_by_label_with_int_length,
            result_set_update_character_stream_with_length,
            result_set_update_character_stream_by_label_with_length,
            JdbcReader,
            "updateCharacterStream"
        ),
    );

    long_stream_update_filter_methods!((
        result_set_update_n_character_stream,
        result_set_update_n_character_stream_by_label,
        result_set_update_n_character_stream_with_length,
        result_set_update_n_character_stream_by_label_with_length,
        JdbcReader,
        "updateNCharacterStream"
    ));

    no_arg_result_set_filter_methods!(
        (result_set_was_null, result_set_was_null, bool, "wasNull"),
        (result_set_previous, result_set_previous, bool, "previous"),
        (result_set_is_before_first, result_set_is_before_first, bool, "isBeforeFirst"),
        (result_set_is_after_last, result_set_is_after_last, bool, "isAfterLast"),
        (result_set_is_first, result_set_is_first, bool, "isFirst"),
        (result_set_is_last, result_set_is_last, bool, "isLast"),
        (result_set_before_first, result_set_before_first, (), "beforeFirst"),
        (result_set_after_last, result_set_after_last, (), "afterLast"),
        (result_set_first, result_set_first, bool, "first"),
        (result_set_last, result_set_last, bool, "last"),
        (result_set_get_row, result_set_get_row, i32, "getRow"),
        (
            result_set_get_fetch_direction,
            result_set_get_fetch_direction,
            i32,
            "getFetchDirection"
        ),
        (result_set_get_fetch_size, result_set_get_fetch_size, i32, "getFetchSize"),
        (result_set_get_type, result_set_get_type, i32, "getType"),
        (
            result_set_get_concurrency,
            result_set_get_concurrency,
            i32,
            "getConcurrency"
        ),
        (
            result_set_get_holdability,
            result_set_get_holdability,
            i32,
            "getHoldability"
        ),
        (
            result_set_get_cursor_name,
            result_set_get_cursor_name,
            Option<String>,
            "getCursorName"
        ),
        (result_set_row_updated, result_set_row_updated, bool, "rowUpdated"),
        (result_set_row_inserted, result_set_row_inserted, bool, "rowInserted"),
        (result_set_row_deleted, result_set_row_deleted, bool, "rowDeleted"),
        (result_set_insert_row, result_set_insert_row, (), "insertRow"),
        (result_set_update_row, result_set_update_row, (), "updateRow"),
        (result_set_delete_row, result_set_delete_row, (), "deleteRow"),
        (result_set_refresh_row, result_set_refresh_row, (), "refreshRow"),
        (
            result_set_cancel_row_updates,
            result_set_cancel_row_updates,
            (),
            "cancelRowUpdates"
        ),
        (
            result_set_move_to_insert_row,
            result_set_move_to_insert_row,
            (),
            "moveToInsertRow"
        ),
        (
            result_set_move_to_current_row,
            result_set_move_to_current_row,
            (),
            "moveToCurrentRow"
        ),
        (result_set_is_closed, result_set_is_closed, bool, "isClosed"),
    );

    i32_arg_result_set_filter_methods!(
        (
            result_set_absolute,
            result_set_absolute,
            row,
            bool,
            "absolute"
        ),
        (
            result_set_relative,
            result_set_relative,
            rows,
            bool,
            "relative"
        ),
        (
            result_set_set_fetch_direction,
            result_set_set_fetch_direction,
            direction,
            (),
            "setFetchDirection"
        ),
        (
            result_set_set_fetch_size,
            result_set_set_fetch_size,
            rows,
            (),
            "setFetchSize"
        ),
    );

    /// 包围 Java `ResultSet#findColumn(String)`，保留标签参数。
    fn result_set_find_column(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<usize, DruidError> {
        chain.result_set_find_column(column_label)
    }

    /// 包围 Java `ResultSet#getMetaData()`，保留 metadata 平台句柄。
    fn result_set_get_meta_data(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<ResultSetMetaData, DruidError> {
        chain.result_set_get_meta_data()
    }

    /// 包围 Java `ResultSet#getStatement()`，保留动态 Statement 身份。
    fn result_set_get_statement(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<ResultSetStatement, DruidError> {
        chain.result_set_get_statement()
    }
}
