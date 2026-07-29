//! `ResultSet Filter` 调用链。
//!
//! 对应 Java：`com.alibaba.druid.filter.FilterChainImpl` 的 `resultSet_*` 分派。

use super::{
    DruidError, JdbcArray, JdbcBlob, JdbcCalendarArgument, JdbcClob, JdbcInputStream, JdbcNClob,
    JdbcObject, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcTargetType, JdbcTypeMap, JdbcUrl,
    PhysicalResultSet, ResultSetFilter, ResultSetFilterContext, SqlWarning, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::sync::Arc;

macro_rules! scalar_getter_chain_methods {
    ($(($index:ident, $label:ident, $filter_index:ident, $filter_label:ident, $physical_index:ident, $physical_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int)`，末端调用物理同名重载。")]
            pub fn $index(&mut self, column_index: usize) -> Result<$ty, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index(self, column_index)
                } else {
                    self.physical.$physical_index(column_index)
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String)`，末端调用物理标签重载。")]
            pub fn $label(&mut self, column_label: &str) -> Result<$ty, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label(self, column_label)
                } else {
                    self.physical.$physical_label(column_label)
                }
            }
        )+
    };
}

macro_rules! temporal_getter_chain_methods {
    ($(($index:ident, $label:ident, $index_calendar:ident, $label_calendar:ident, $filter_index:ident, $filter_label:ident, $filter_index_calendar:ident, $filter_label_calendar:ident, $physical_index:ident, $physical_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int)`，末端调用无 Calendar 物理重载。")]
            pub fn $index(&mut self, column_index: usize) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index(self, column_index)
                } else {
                    self.physical
                        .$physical_index(column_index, &JdbcCalendarArgument::unspecified())
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String)`，末端调用无 Calendar 物理标签重载。")]
            pub fn $label(&mut self, column_label: &str) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label(self, column_label)
                } else {
                    self.physical.$physical_label(
                        column_label,
                        &JdbcCalendarArgument::unspecified(),
                    )
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int, Calendar)`，末端保留 Calendar 重载身份。")]
            pub fn $index_calendar(
                &mut self,
                column_index: usize,
                calendar: &JdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index_calendar(self, column_index, calendar)
                } else {
                    self.physical.$physical_index(column_index, calendar)
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String, Calendar)`，末端保留 Calendar 重载身份。")]
            pub fn $label_calendar(
                &mut self,
                column_label: &str,
                calendar: &JdbcCalendarArgument,
            ) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label_calendar(self, column_label, calendar)
                } else {
                    self.physical.$physical_label(column_label, calendar)
                }
            }
        )+
    };
}

macro_rules! resource_getter_chain_methods {
    ($(($index:ident, $label:ident, $filter_index:ident, $filter_label:ident, $physical_index:ident, $physical_label:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int)`，末端返回物理资源句柄。")]
            pub fn $index(&mut self, column_index: usize) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_index(self, column_index)
                } else {
                    self.physical.$physical_index(column_index)
                }
            }

            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(String)`，末端调用物理标签重载。")]
            pub fn $label(&mut self, column_label: &str) -> Result<Option<$ty>, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_label(self, column_label)
                } else {
                    self.physical.$physical_label(column_label)
                }
            }
        )+
    };
}

macro_rules! no_arg_result_set_chain_methods {
    ($(($method:ident, $filter_method:ident, $physical_method:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "()`，末端调用物理方法。")]
            pub fn $method(&mut self) -> Result<$ty, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_method(self)
                } else {
                    self.physical.$physical_method()
                }
            }
        )+
    };
}

macro_rules! i32_arg_result_set_chain_methods {
    ($(($method:ident, $filter_method:ident, $physical_method:ident, $argument:ident, $ty:ty, $java:literal)),+ $(,)?) => {
        $(
            #[doc = concat!("继续分派 Java `ResultSet#", $java, "(int)`，末端保留参数身份。")]
            pub fn $method(&mut self, $argument: i32) -> Result<$ty, DruidError> {
                if self.position < self.filters.len() {
                    let filter = Arc::clone(&self.filters[self.position]);
                    self.position += 1;
                    filter.$filter_method(self, $argument)
                } else {
                    self.physical.$physical_method($argument)
                }
            }
        )+
    };
}

/// 单次 `ResultSet` 操作使用的有位置调用链。
///
/// 每次 `ResultSet` 方法调用都创建新链并从位置 0 开始，等价于 Java
/// `ResultSetProxyImpl#createChain()` 与 `recycleFilterChain(reset)`。
pub struct ResultSetFilterChain<'a> {
    filters: &'a [Arc<dyn ResultSetFilter>],
    position: usize,
    physical: &'a dyn PhysicalResultSet,
    context: &'a ResultSetFilterContext,
}

impl<'a> ResultSetFilterChain<'a> {
    /// 创建从第一个 Filter 开始的单次调用链。
    pub fn new(
        filters: &'a [Arc<dyn ResultSetFilter>],
        physical: &'a dyn PhysicalResultSet,
        context: &'a ResultSetFilterContext,
    ) -> Self {
        Self {
            filters,
            position: 0,
            physical,
            context,
        }
    }

    /// 继续分派 `ResultSet#next()`，末端调用物理结果集。
    pub fn result_set_next(&mut self) -> Result<bool, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_next(self)
        } else {
            self.physical.next()
        }
    }

    /// 继续分派 `ResultSet#close()`，末端调用物理结果集。
    pub fn result_set_close(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_close(self)
        } else {
            self.physical.close()
        }
    }

    /// 继续分派 `ResultSet#getWarnings()`，末端调用物理结果集。
    pub fn result_set_get_warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_warnings(self)
        } else {
            self.physical.warnings()
        }
    }

    /// 继续分派 `ResultSet#clearWarnings()`，末端调用物理结果集。
    pub fn result_set_clear_warnings(&mut self) -> Result<(), DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_clear_warnings(self)
        } else {
            self.physical.clear_warnings()
        }
    }

    /// 继续分派 Java `ResultSet#getObject(int)`，末端调用物理下标重载。
    pub fn result_set_get_object(&mut self, column_index: usize) -> Result<Value, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object(self, column_index)
        } else {
            self.physical.value(column_index)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(String)`，末端调用物理标签重载。
    pub fn result_set_get_object_by_label(
        &mut self,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_by_label(self, column_label)
        } else {
            self.physical.value_by_label(column_label)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(int, Map)`，保留 `null` Map。
    pub fn result_set_get_object_with_type_map(
        &mut self,
        column_index: usize,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_with_type_map(self, column_index, type_map)
        } else {
            self.physical.object_with_type_map(column_index, type_map)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(String, Map)`，保持标签重载身份。
    pub fn result_set_get_object_by_label_with_type_map(
        &mut self,
        column_label: &str,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_by_label_with_type_map(self, column_label, type_map)
        } else {
            self.physical
                .object_by_label_with_type_map(column_label, type_map)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(int, Class<T>)`。
    pub fn result_set_get_object_typed(
        &mut self,
        column_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_typed(self, column_index, target_type)
        } else {
            self.physical.object_as(column_index, target_type)
        }
    }

    /// 继续分派 Java `ResultSet#getObject(String, Class<T>)`。
    pub fn result_set_get_object_typed_by_label(
        &mut self,
        column_label: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_object_typed_by_label(self, column_label, target_type)
        } else {
            self.physical.object_by_label_as(column_label, target_type)
        }
    }

    scalar_getter_chain_methods!(
        (
            result_set_get_string,
            result_set_get_string_by_label,
            result_set_get_string,
            result_set_get_string_by_label,
            string,
            string_by_label,
            Option<String>,
            "getString"
        ),
        (
            result_set_get_boolean,
            result_set_get_boolean_by_label,
            result_set_get_boolean,
            result_set_get_boolean_by_label,
            boolean,
            boolean_by_label,
            bool,
            "getBoolean"
        ),
        (
            result_set_get_byte,
            result_set_get_byte_by_label,
            result_set_get_byte,
            result_set_get_byte_by_label,
            byte,
            byte_by_label,
            i8,
            "getByte"
        ),
        (
            result_set_get_short,
            result_set_get_short_by_label,
            result_set_get_short,
            result_set_get_short_by_label,
            short,
            short_by_label,
            i16,
            "getShort"
        ),
        (
            result_set_get_int,
            result_set_get_int_by_label,
            result_set_get_int,
            result_set_get_int_by_label,
            int,
            int_by_label,
            i32,
            "getInt"
        ),
        (
            result_set_get_long,
            result_set_get_long_by_label,
            result_set_get_long,
            result_set_get_long_by_label,
            long,
            long_by_label,
            i64,
            "getLong"
        ),
        (
            result_set_get_float,
            result_set_get_float_by_label,
            result_set_get_float,
            result_set_get_float_by_label,
            float,
            float_by_label,
            f32,
            "getFloat"
        ),
        (
            result_set_get_double,
            result_set_get_double_by_label,
            result_set_get_double,
            result_set_get_double_by_label,
            double,
            double_by_label,
            f64,
            "getDouble"
        ),
        (
            result_set_get_bytes,
            result_set_get_bytes_by_label,
            result_set_get_bytes,
            result_set_get_bytes_by_label,
            bytes,
            bytes_by_label,
            Option<Vec<u8>>,
            "getBytes"
        ),
        (
            result_set_get_n_string,
            result_set_get_n_string_by_label,
            result_set_get_n_string,
            result_set_get_n_string_by_label,
            n_string,
            n_string_by_label,
            Option<String>,
            "getNString"
        ),
    );

    /// 继续分派 Java `ResultSet#getBigDecimal(int)`。
    pub fn result_set_get_big_decimal(
        &mut self,
        column_index: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_big_decimal(self, column_index)
        } else {
            self.physical.big_decimal(column_index, None)
        }
    }

    /// 继续分派 Java `ResultSet#getBigDecimal(String)`。
    pub fn result_set_get_big_decimal_by_label(
        &mut self,
        column_label: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_big_decimal_by_label(self, column_label)
        } else {
            self.physical.big_decimal_by_label(column_label, None)
        }
    }

    /// 继续分派 Java `ResultSet#getBigDecimal(int, int)`。
    pub fn result_set_get_big_decimal_with_scale(
        &mut self,
        column_index: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_big_decimal_with_scale(self, column_index, scale)
        } else {
            self.physical.big_decimal(column_index, Some(scale))
        }
    }

    /// 继续分派 Java `ResultSet#getBigDecimal(String, int)`。
    pub fn result_set_get_big_decimal_by_label_with_scale(
        &mut self,
        column_label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_get_big_decimal_by_label_with_scale(self, column_label, scale)
        } else {
            self.physical
                .big_decimal_by_label(column_label, Some(scale))
        }
    }

    temporal_getter_chain_methods!(
        (
            result_set_get_date,
            result_set_get_date_by_label,
            result_set_get_date_with_calendar,
            result_set_get_date_by_label_with_calendar,
            result_set_get_date,
            result_set_get_date_by_label,
            result_set_get_date_with_calendar,
            result_set_get_date_by_label_with_calendar,
            date,
            date_by_label,
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
            time,
            time_by_label,
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
            timestamp,
            timestamp_by_label,
            NaiveDateTime,
            "getTimestamp"
        ),
    );

    resource_getter_chain_methods!(
        (
            result_set_get_ref,
            result_set_get_ref_by_label,
            result_set_get_ref,
            result_set_get_ref_by_label,
            reference,
            reference_by_label,
            JdbcRef,
            "getRef"
        ),
        (
            result_set_get_blob,
            result_set_get_blob_by_label,
            result_set_get_blob,
            result_set_get_blob_by_label,
            blob,
            blob_by_label,
            JdbcBlob,
            "getBlob"
        ),
        (
            result_set_get_clob,
            result_set_get_clob_by_label,
            result_set_get_clob,
            result_set_get_clob_by_label,
            clob,
            clob_by_label,
            JdbcClob,
            "getClob"
        ),
        (
            result_set_get_array,
            result_set_get_array_by_label,
            result_set_get_array,
            result_set_get_array_by_label,
            array,
            array_by_label,
            JdbcArray,
            "getArray"
        ),
        (
            result_set_get_url,
            result_set_get_url_by_label,
            result_set_get_url,
            result_set_get_url_by_label,
            url,
            url_by_label,
            JdbcUrl,
            "getURL"
        ),
        (
            result_set_get_row_id,
            result_set_get_row_id_by_label,
            result_set_get_row_id,
            result_set_get_row_id_by_label,
            row_id,
            row_id_by_label,
            JdbcRowId,
            "getRowId"
        ),
        (
            result_set_get_n_clob,
            result_set_get_n_clob_by_label,
            result_set_get_n_clob,
            result_set_get_n_clob_by_label,
            n_clob,
            n_clob_by_label,
            JdbcNClob,
            "getNClob"
        ),
        (
            result_set_get_sql_xml,
            result_set_get_sql_xml_by_label,
            result_set_get_sql_xml,
            result_set_get_sql_xml_by_label,
            sql_xml,
            sql_xml_by_label,
            JdbcSqlXml,
            "getSQLXML"
        ),
        (
            result_set_get_ascii_stream,
            result_set_get_ascii_stream_by_label,
            result_set_get_ascii_stream,
            result_set_get_ascii_stream_by_label,
            ascii_stream,
            ascii_stream_by_label,
            JdbcInputStream,
            "getAsciiStream"
        ),
        (
            result_set_get_unicode_stream,
            result_set_get_unicode_stream_by_label,
            result_set_get_unicode_stream,
            result_set_get_unicode_stream_by_label,
            unicode_stream,
            unicode_stream_by_label,
            JdbcInputStream,
            "getUnicodeStream"
        ),
        (
            result_set_get_binary_stream,
            result_set_get_binary_stream_by_label,
            result_set_get_binary_stream,
            result_set_get_binary_stream_by_label,
            binary_stream,
            binary_stream_by_label,
            JdbcInputStream,
            "getBinaryStream"
        ),
        (
            result_set_get_character_stream,
            result_set_get_character_stream_by_label,
            result_set_get_character_stream,
            result_set_get_character_stream_by_label,
            character_stream,
            character_stream_by_label,
            JdbcReader,
            "getCharacterStream"
        ),
        (
            result_set_get_n_character_stream,
            result_set_get_n_character_stream_by_label,
            result_set_get_n_character_stream,
            result_set_get_n_character_stream_by_label,
            n_character_stream,
            n_character_stream_by_label,
            JdbcReader,
            "getNCharacterStream"
        ),
    );

    no_arg_result_set_chain_methods!(
        (result_set_was_null, result_set_was_null, was_null, bool, "wasNull"),
        (result_set_previous, result_set_previous, previous, bool, "previous"),
        (result_set_is_before_first, result_set_is_before_first, is_before_first, bool, "isBeforeFirst"),
        (result_set_is_after_last, result_set_is_after_last, is_after_last, bool, "isAfterLast"),
        (result_set_is_first, result_set_is_first, is_first, bool, "isFirst"),
        (result_set_is_last, result_set_is_last, is_last, bool, "isLast"),
        (result_set_before_first, result_set_before_first, before_first, (), "beforeFirst"),
        (result_set_after_last, result_set_after_last, after_last, (), "afterLast"),
        (result_set_first, result_set_first, first, bool, "first"),
        (result_set_last, result_set_last, last, bool, "last"),
        (result_set_get_row, result_set_get_row, row, i32, "getRow"),
        (result_set_get_fetch_direction, result_set_get_fetch_direction, fetch_direction, i32, "getFetchDirection"),
        (result_set_get_fetch_size, result_set_get_fetch_size, fetch_size, i32, "getFetchSize"),
        (result_set_get_type, result_set_get_type, result_set_type, i32, "getType"),
        (result_set_get_concurrency, result_set_get_concurrency, concurrency, i32, "getConcurrency"),
        (result_set_get_holdability, result_set_get_holdability, holdability, i32, "getHoldability"),
        (result_set_get_cursor_name, result_set_get_cursor_name, cursor_name, Option<String>, "getCursorName"),
        (result_set_row_updated, result_set_row_updated, row_updated, bool, "rowUpdated"),
        (result_set_row_inserted, result_set_row_inserted, row_inserted, bool, "rowInserted"),
        (result_set_row_deleted, result_set_row_deleted, row_deleted, bool, "rowDeleted"),
    );

    i32_arg_result_set_chain_methods!(
        (
            result_set_absolute,
            result_set_absolute,
            absolute,
            row,
            bool,
            "absolute"
        ),
        (
            result_set_relative,
            result_set_relative,
            relative,
            rows,
            bool,
            "relative"
        ),
        (
            result_set_set_fetch_direction,
            result_set_set_fetch_direction,
            set_fetch_direction,
            direction,
            (),
            "setFetchDirection"
        ),
        (
            result_set_set_fetch_size,
            result_set_set_fetch_size,
            set_fetch_size,
            rows,
            (),
            "setFetchSize"
        ),
    );

    /// 继续分派 Java `ResultSet#findColumn(String)`。
    pub fn result_set_find_column(&mut self, column_label: &str) -> Result<usize, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_find_column(self, column_label)
        } else {
            self.physical.find_column(column_label)
        }
    }

    /// 继续分派 Java `ResultSet#isClosed()`。
    pub fn result_set_is_closed(&mut self) -> Result<bool, DruidError> {
        if self.position < self.filters.len() {
            let filter = Arc::clone(&self.filters[self.position]);
            self.position += 1;
            filter.result_set_is_closed(self)
        } else {
            Ok(self.physical.is_closed())
        }
    }

    /// 返回本结果集共享的 Filter 上下文。
    pub fn context(&self) -> &ResultSetFilterContext {
        self.context
    }
}
