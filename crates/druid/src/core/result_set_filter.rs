//! `ResultSet` 同步 `Filter` 协议。
//!
//! 对应 Java：`com.alibaba.druid.filter.Filter` 的 `resultSet_*` 方法族。

use super::{
    DruidError, JdbcArray, JdbcBlob, JdbcCalendarArgument, JdbcClob, JdbcInputStream, JdbcNClob,
    JdbcObject, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcTargetType, JdbcTypeMap, JdbcUrl,
    ResultSetFilterChain, ResultSetFilterContext, SqlWarning, Value,
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
}
