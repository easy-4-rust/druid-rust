//! Java `ResultSet FilterChain` 与 `StatFilter` 的顺序、短路及真实 `SQLite` 契约。

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    DruidError, DruidPooledConnection, FilterChain, JdbcArray, JdbcBlob, JdbcCalendar,
    JdbcCalendarArgument, JdbcCharacterLength, JdbcClob, JdbcInputStream, JdbcNClob, JdbcObject,
    JdbcOpaqueObject, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcStreamLength, JdbcTargetType,
    JdbcTypeMap, JdbcUrl, PhysicalConnectionFactory, PhysicalJdbcOpaqueObject, PhysicalResultSet,
    ResultSetFilter, ResultSetFilterChain, ResultSetFilterContext, ResultSetMetaData,
    ResultSetStatement, ResultSetUpdate, Value,
};
use druid::stats::{StatFilter, StatsCollector};
use druid::toasty::ToastyConnectionFactory;
use std::any::Any;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

macro_rules! physical_scalar_getter_pair {
    ($index:ident, $label:ident, $ty:ty, $index_value:expr, $label_value:expr) => {
        fn $index(&self, column_index: usize) -> Result<$ty, DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("physical:{}:{column_index}", stringify!($index)));
            Ok($index_value)
        }

        fn $label(&self, column_label: &str) -> Result<$ty, DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("physical:{}:{column_label}", stringify!($label)));
            Ok($label_value)
        }
    };
}

fn calendar_identity(calendar: &JdbcCalendarArgument) -> String {
    match calendar {
        JdbcCalendarArgument::Unspecified => "unspecified".to_string(),
        JdbcCalendarArgument::Specified(None) => "specified:null".to_string(),
        JdbcCalendarArgument::Specified(Some(calendar)) => {
            format!("specified:{}", calendar.time_zone_id())
        }
    }
}

fn type_map_identity(type_map: Option<&JdbcTypeMap>) -> String {
    match type_map {
        None => "null".to_string(),
        Some(type_map) if type_map.is_empty() => "empty".to_string(),
        Some(type_map) => {
            let mut entries = type_map
                .mappings()
                .iter()
                .map(|(name, target)| format!("{name}={target:?}"))
                .collect::<Vec<_>>();
            entries.sort();
            entries.join(",")
        }
    }
}

fn result_set_update_identity(update: &ResultSetUpdate) -> String {
    match update {
        ResultSetUpdate::AsciiStream { stream, length } => {
            format!("AsciiStream:{}:{length:?}", stream.is_some())
        }
        ResultSetUpdate::BinaryStream { stream, length } => {
            format!("BinaryStream:{}:{length:?}", stream.is_some())
        }
        ResultSetUpdate::CharacterStream { reader, length } => {
            format!("CharacterStream:{}:{length:?}", reader.is_some())
        }
        ResultSetUpdate::NCharacterStream { reader, length } => {
            format!("NCharacterStream:{}:{length:?}", reader.is_some())
        }
        _ => format!("{update:?}"),
    }
}

macro_rules! physical_temporal_getter_pair {
    ($index:ident, $label:ident, $ty:ty, $index_value:expr, $label_value:expr) => {
        fn $index(
            &self,
            column_index: usize,
            calendar: &JdbcCalendarArgument,
        ) -> Result<Option<$ty>, DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!(
                    "physical:{}:{column_index}:{}",
                    stringify!($index),
                    calendar_identity(calendar)
                ));
            Ok(Some($index_value))
        }

        fn $label(
            &self,
            column_label: &str,
            calendar: &JdbcCalendarArgument,
        ) -> Result<Option<$ty>, DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!(
                    "physical:{}:{column_label}:{}",
                    stringify!($label),
                    calendar_identity(calendar)
                ));
            Ok(Some($label_value))
        }
    };
}

macro_rules! physical_resource_getter_pair {
    ($index:ident, $label:ident, $ty:ty) => {
        fn $index(&self, column_index: usize) -> Result<Option<$ty>, DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("physical:{}:{column_index}", stringify!($index)));
            Ok(None)
        }

        fn $label(&self, column_label: &str) -> Result<Option<$ty>, DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("physical:{}:{column_label}", stringify!($label)));
            Ok(None)
        }
    };
}

macro_rules! physical_resource_update_pair {
    ($index:ident, $label:ident, $ty:ty) => {
        fn $index(&self, column_index: usize, value: Option<&$ty>) -> Result<(), DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!(
                    "physical:{}:{column_index}:{value:?}",
                    stringify!($index)
                ));
            Ok(())
        }

        fn $label(&self, column_label: &str, value: Option<&$ty>) -> Result<(), DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!(
                    "physical:{}:{column_label}:{value:?}",
                    stringify!($label)
                ));
            Ok(())
        }
    };
}

macro_rules! physical_lob_stream_update_pair {
    ($index:ident, $label:ident, $ty:ty, $length_ty:ty) => {
        fn $index(
            &self,
            column_index: usize,
            value: Option<&$ty>,
            length: $length_ty,
        ) -> Result<(), DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!(
                    "physical:{}:{column_index}:{}:{length:?}",
                    stringify!($index),
                    value.is_some()
                ));
            Ok(())
        }

        fn $label(
            &self,
            column_label: &str,
            value: Option<&$ty>,
            length: $length_ty,
        ) -> Result<(), DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!(
                    "physical:{}:{column_label}:{}:{length:?}",
                    stringify!($label),
                    value.is_some()
                ));
            Ok(())
        }
    };
}

macro_rules! physical_no_arg_method {
    ($method:ident, $ty:ty, $value:expr) => {
        fn $method(&self) -> Result<$ty, DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("physical:{}", stringify!($method)));
            Ok($value)
        }
    };
}

macro_rules! physical_i32_arg_method {
    ($method:ident, $argument:ident, $ty:ty, $value:expr) => {
        fn $method(&self, $argument: i32) -> Result<$ty, DruidError> {
            self.calls
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(format!("physical:{}:{}", stringify!($method), $argument));
            Ok($value)
        }
    };
}

#[derive(Debug)]
struct PhysicalResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: AtomicBool,
    next_count: AtomicU64,
    close_error: bool,
}

impl PhysicalResultSetProbe {
    fn new(calls: Arc<Mutex<Vec<String>>>, close_error: bool) -> Self {
        Self {
            calls,
            closed: AtomicBool::new(false),
            next_count: AtomicU64::new(0),
            close_error,
        }
    }
}

impl PhysicalResultSet for PhysicalResultSetProbe {
    fn close(&self) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("physical:close".to_string());
        self.closed.store(true, Ordering::Release);
        if self.close_error {
            Err(DruidError::DriverError("close failed".to_string()))
        } else {
            Ok(())
        }
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn next(&self) -> Result<bool, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("physical:next".to_string());
        self.next_count.fetch_add(1, Ordering::AcqRel);
        Ok(true)
    }

    fn value(&self, column_index: usize) -> Result<Value, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("physical:value:{column_index}"));
        Ok(Value::Int(i64::try_from(column_index).unwrap()))
    }

    fn value_by_label(&self, column_label: &str) -> Result<Value, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("physical:value_by_label:{column_label}"));
        Ok(Value::String(column_label.to_string()))
    }

    fn object_with_type_map(
        &self,
        column_index: usize,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        let identity = type_map_identity(type_map);
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "physical:object_with_type_map:{column_index}:{identity}"
            ));
        Ok(JdbcObject::String(format!(
            "index-map:{column_index}:{identity}"
        )))
    }

    fn object_by_label_with_type_map(
        &self,
        column_label: &str,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        let identity = type_map_identity(type_map);
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "physical:object_by_label_with_type_map:{column_label}:{identity}"
            ));
        Ok(JdbcObject::String(format!(
            "label-map:{column_label}:{identity}"
        )))
    }

    fn object_as(
        &self,
        column_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("physical:object_as:{column_index}:{target_type:?}"));
        Ok(JdbcObject::String(format!(
            "index-typed:{column_index}:{target_type:?}"
        )))
    }

    fn object_by_label_as(
        &self,
        column_label: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "physical:object_by_label_as:{column_label}:{target_type:?}"
            ));
        Ok(JdbcObject::String(format!(
            "label-typed:{column_label}:{target_type:?}"
        )))
    }

    physical_scalar_getter_pair!(
        string,
        string_by_label,
        Option<String>,
        Some("index-string".to_string()),
        Some("label-string".to_string())
    );
    physical_scalar_getter_pair!(
        n_string,
        n_string_by_label,
        Option<String>,
        Some("索引-NString".to_string()),
        Some("标签-NString".to_string())
    );
    physical_scalar_getter_pair!(boolean, boolean_by_label, bool, true, false);
    physical_scalar_getter_pair!(byte, byte_by_label, i8, 8, 9);
    physical_scalar_getter_pair!(short, short_by_label, i16, 16, 17);
    physical_scalar_getter_pair!(int, int_by_label, i32, 32, 33);
    physical_scalar_getter_pair!(long, long_by_label, i64, 64, 65);
    physical_scalar_getter_pair!(float, float_by_label, f32, 1.25, 2.25);
    physical_scalar_getter_pair!(double, double_by_label, f64, 3.5, 4.5);
    physical_scalar_getter_pair!(
        bytes,
        bytes_by_label,
        Option<Vec<u8>>,
        Some(vec![1, 2]),
        Some(vec![3, 4])
    );

    fn big_decimal(
        &self,
        column_index: usize,
        scale: Option<i32>,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("physical:big_decimal:{column_index}:{scale:?}"));
        Ok(Some(BigDecimal::from_str("12.340").unwrap()))
    }

    fn big_decimal_by_label(
        &self,
        column_label: &str,
        scale: Option<i32>,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "physical:big_decimal_by_label:{column_label}:{scale:?}"
            ));
        Ok(Some(BigDecimal::from_str("56.780").unwrap()))
    }

    physical_temporal_getter_pair!(
        date,
        date_by_label,
        NaiveDate,
        NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
        NaiveDate::from_ymd_opt(2025, 2, 3).unwrap()
    );
    physical_temporal_getter_pair!(
        time,
        time_by_label,
        NaiveTime,
        NaiveTime::from_hms_opt(3, 4, 5).unwrap(),
        NaiveTime::from_hms_opt(6, 7, 8).unwrap()
    );
    physical_temporal_getter_pair!(
        timestamp,
        timestamp_by_label,
        NaiveDateTime,
        NaiveDate::from_ymd_opt(2025, 1, 2)
            .unwrap()
            .and_hms_opt(3, 4, 5)
            .unwrap(),
        NaiveDate::from_ymd_opt(2025, 2, 3)
            .unwrap()
            .and_hms_opt(6, 7, 8)
            .unwrap()
    );
    physical_resource_getter_pair!(reference, reference_by_label, JdbcRef);
    physical_resource_getter_pair!(blob, blob_by_label, JdbcBlob);
    physical_resource_getter_pair!(clob, clob_by_label, JdbcClob);
    physical_resource_getter_pair!(array, array_by_label, JdbcArray);
    physical_resource_getter_pair!(url, url_by_label, JdbcUrl);
    physical_resource_getter_pair!(row_id, row_id_by_label, JdbcRowId);
    physical_resource_getter_pair!(n_clob, n_clob_by_label, JdbcNClob);
    physical_resource_getter_pair!(sql_xml, sql_xml_by_label, JdbcSqlXml);
    physical_resource_getter_pair!(ascii_stream, ascii_stream_by_label, JdbcInputStream);
    physical_resource_getter_pair!(unicode_stream, unicode_stream_by_label, JdbcInputStream);
    physical_resource_getter_pair!(binary_stream, binary_stream_by_label, JdbcInputStream);
    physical_resource_getter_pair!(character_stream, character_stream_by_label, JdbcReader);
    physical_resource_getter_pair!(n_character_stream, n_character_stream_by_label, JdbcReader);
    physical_resource_update_pair!(update_reference, update_reference_by_label, JdbcRef);
    physical_resource_update_pair!(update_blob, update_blob_by_label, JdbcBlob);
    physical_resource_update_pair!(update_clob, update_clob_by_label, JdbcClob);
    physical_resource_update_pair!(update_array, update_array_by_label, JdbcArray);
    physical_resource_update_pair!(update_row_id, update_row_id_by_label, JdbcRowId);
    physical_resource_update_pair!(update_n_clob, update_n_clob_by_label, JdbcNClob);
    physical_resource_update_pair!(update_sql_xml, update_sql_xml_by_label, JdbcSqlXml);
    physical_lob_stream_update_pair!(
        update_blob_stream,
        update_blob_stream_by_label,
        JdbcInputStream,
        JdbcStreamLength
    );
    physical_lob_stream_update_pair!(
        update_clob_reader,
        update_clob_reader_by_label,
        JdbcReader,
        JdbcCharacterLength
    );
    physical_lob_stream_update_pair!(
        update_n_clob_reader,
        update_n_clob_reader_by_label,
        JdbcReader,
        JdbcCharacterLength
    );
    physical_no_arg_method!(was_null, bool, true);
    physical_no_arg_method!(previous, bool, false);
    physical_no_arg_method!(is_before_first, bool, true);
    physical_no_arg_method!(is_after_last, bool, false);
    physical_no_arg_method!(is_first, bool, false);
    physical_no_arg_method!(is_last, bool, true);
    physical_no_arg_method!(before_first, (), ());
    physical_no_arg_method!(after_last, (), ());
    physical_no_arg_method!(first, bool, true);
    physical_no_arg_method!(last, bool, true);
    physical_no_arg_method!(row, i32, 7);
    physical_no_arg_method!(fetch_direction, i32, 1000);
    physical_no_arg_method!(fetch_size, i32, 64);
    physical_no_arg_method!(result_set_type, i32, 1004);
    physical_no_arg_method!(concurrency, i32, 1007);
    physical_no_arg_method!(holdability, i32, 1);
    physical_no_arg_method!(cursor_name, Option<String>, Some("cursor".to_string()));
    physical_no_arg_method!(row_updated, bool, true);
    physical_no_arg_method!(row_inserted, bool, false);
    physical_no_arg_method!(row_deleted, bool, true);
    physical_no_arg_method!(insert_row, (), ());
    physical_no_arg_method!(update_row, (), ());
    physical_no_arg_method!(delete_row, (), ());
    physical_no_arg_method!(refresh_row, (), ());
    physical_no_arg_method!(cancel_row_updates, (), ());
    physical_no_arg_method!(move_to_insert_row, (), ());
    physical_no_arg_method!(move_to_current_row, (), ());
    physical_i32_arg_method!(absolute, row, bool, row == 5);
    physical_i32_arg_method!(relative, rows, bool, rows == -2);
    physical_i32_arg_method!(set_fetch_direction, direction, (), ());
    physical_i32_arg_method!(set_fetch_size, rows, (), ());

    fn find_column(&self, column_label: &str) -> Result<usize, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("physical:find_column:{column_label}"));
        Ok(23)
    }

    fn meta_data(&self) -> Result<ResultSetMetaData, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("physical:meta_data".to_string());
        Ok(ResultSetMetaData::new(Vec::new()))
    }

    fn update_value(
        &self,
        column_index: usize,
        update: &ResultSetUpdate,
    ) -> Result<(), DruidError> {
        let identity = result_set_update_identity(update);
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("physical:update_value:{column_index}:{identity}"));
        Ok(())
    }

    fn update_value_by_label(
        &self,
        column_label: &str,
        update: &ResultSetUpdate,
    ) -> Result<(), DruidError> {
        let identity = result_set_update_identity(update);
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "physical:update_value_by_label:{column_label}:{identity}"
            ));
        Ok(())
    }
}

struct OrderingResultSetFilter {
    label: &'static str,
    calls: Arc<Mutex<Vec<String>>>,
    short_circuit_next: bool,
}

struct PassThroughResultSetFilter;

impl ResultSetFilter for PassThroughResultSetFilter {}

struct IntWrappingFilter {
    label: &'static str,
    delta: i32,
    calls: Arc<Mutex<Vec<String>>>,
}

impl ResultSetFilter for IntWrappingFilter {
    fn result_set_get_int(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<i32, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}:int-before:{column_index}", self.label));
        let value = chain.result_set_get_int(column_index)?;
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}:int-after:{value}", self.label));
        Ok(value + self.delta)
    }
}

struct ScalarShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

impl ScalarShortCircuitFilter {
    fn record_index(&self, operation: &str, column_index: usize) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{operation}:{column_index}"));
    }

    fn record_label(&self, operation: &str, column_label: &str) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{operation}:{column_label}"));
    }
}

macro_rules! short_circuit_scalar_filter_pair {
    ($index:ident, $label:ident, $ty:ty, $index_value:expr, $label_value:expr) => {
        fn $index(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
        ) -> Result<$ty, DruidError> {
            self.record_index(stringify!($index), column_index);
            Ok($index_value)
        }

        fn $label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
        ) -> Result<$ty, DruidError> {
            self.record_label(stringify!($label), column_label);
            Ok($label_value)
        }
    };
}

impl ResultSetFilter for ScalarShortCircuitFilter {
    short_circuit_scalar_filter_pair!(
        result_set_get_string,
        result_set_get_string_by_label,
        Option<String>,
        Some("filtered-index".to_string()),
        Some("filtered-label".to_string())
    );
    short_circuit_scalar_filter_pair!(
        result_set_get_boolean,
        result_set_get_boolean_by_label,
        bool,
        true,
        false
    );
    short_circuit_scalar_filter_pair!(
        result_set_get_byte,
        result_set_get_byte_by_label,
        i8,
        11,
        12
    );
    short_circuit_scalar_filter_pair!(
        result_set_get_short,
        result_set_get_short_by_label,
        i16,
        21,
        22
    );
    short_circuit_scalar_filter_pair!(result_set_get_int, result_set_get_int_by_label, i32, 31, 32);
    short_circuit_scalar_filter_pair!(
        result_set_get_long,
        result_set_get_long_by_label,
        i64,
        41,
        42
    );
    short_circuit_scalar_filter_pair!(
        result_set_get_float,
        result_set_get_float_by_label,
        f32,
        51.5,
        52.5
    );

    fn result_set_get_double(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<f64, DruidError> {
        self.record_index("result_set_get_double", column_index);
        Ok(61.5)
    }

    fn result_set_get_double_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<f64, DruidError> {
        self.record_label("result_set_get_double_by_label", column_label);
        if column_label == "fail" {
            Err(DruidError::DriverError(
                "filtered double failure".to_string(),
            ))
        } else {
            Ok(62.5)
        }
    }

    short_circuit_scalar_filter_pair!(
        result_set_get_bytes,
        result_set_get_bytes_by_label,
        Option<Vec<u8>>,
        Some(vec![7, 8]),
        Some(vec![9, 10])
    );
}

struct StrongGetterShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

struct ObjectShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

struct ResourceShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
    stream: JdbcInputStream,
    reader: JdbcReader,
}

struct NavigationShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

struct RowMutationShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
    fail_refresh: Arc<AtomicBool>,
}

struct ScalarUpdateShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

struct NStringUpdateShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

struct ObjectUpdateShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
    expected_custom: JdbcOpaqueObject,
}

struct ResourceUpdateShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

struct LobStreamUpdateShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

struct StreamUpdateShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

impl LobStreamUpdateShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

macro_rules! short_circuit_blob_stream_update_family {
    () => {
        fn result_set_update_blob_stream(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<JdbcInputStream>,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            if let Some(stream) = value {
                let mut byte = [0_u8; 1];
                assert_eq!(stream.read(&mut byte)?, 1);
            }
            self.record(format!(
                "result_set_update_blob_stream:{column_index}:{presence}"
            ));
            Ok(())
        }

        fn result_set_update_blob_stream_by_label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<JdbcInputStream>,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "result_set_update_blob_stream_by_label:{column_label}:{presence}"
            ));
            Ok(())
        }

        fn result_set_update_blob_stream_with_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<JdbcInputStream>,
            length: i64,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "result_set_update_blob_stream_with_length:{column_index}:{presence}:{length}"
            ));
            Ok(())
        }

        fn result_set_update_blob_stream_by_label_with_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<JdbcInputStream>,
            length: i64,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "result_set_update_blob_stream_by_label_with_length:{column_label}:{presence}:{length}"
            ));
            Ok(())
        }
    };
}

macro_rules! short_circuit_reader_update_family {
    (
        $index:ident,
        $label:ident,
        $index_with_length:ident,
        $label_with_length:ident
    ) => {
        fn $index(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<JdbcReader>,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            if let Some(reader) = value {
                let mut code_unit = [0_u16; 1];
                assert_eq!(reader.read_utf16(&mut code_unit)?, 1);
            }
            self.record(format!("{}:{column_index}:{presence}", stringify!($index)));
            Ok(())
        }

        fn $label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<JdbcReader>,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!("{}:{column_label}:{presence}", stringify!($label)));
            Ok(())
        }

        fn $index_with_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<JdbcReader>,
            length: i64,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "{}:{column_index}:{presence}:{length}",
                stringify!($index_with_length)
            ));
            Ok(())
        }

        fn $label_with_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<JdbcReader>,
            length: i64,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "{}:{column_label}:{presence}:{length}",
                stringify!($label_with_length)
            ));
            if column_label == "fail" {
                Err(DruidError::DriverError(
                    "filtered LOB stream update failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    };
}

impl ResultSetFilter for LobStreamUpdateShortCircuitFilter {
    short_circuit_blob_stream_update_family!();
    short_circuit_reader_update_family!(
        result_set_update_clob_reader,
        result_set_update_clob_reader_by_label,
        result_set_update_clob_reader_with_length,
        result_set_update_clob_reader_by_label_with_length
    );
    short_circuit_reader_update_family!(
        result_set_update_n_clob_reader,
        result_set_update_n_clob_reader_by_label,
        result_set_update_n_clob_reader_with_length,
        result_set_update_n_clob_reader_by_label_with_length
    );
}

impl StreamUpdateShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

trait ConsumeOneForFilter {
    fn consume_one_for_filter(&self) -> Result<(), DruidError>;
}

impl ConsumeOneForFilter for JdbcInputStream {
    fn consume_one_for_filter(&self) -> Result<(), DruidError> {
        let mut byte = [0_u8; 1];
        assert_eq!(self.read(&mut byte)?, 1);
        Ok(())
    }
}

impl ConsumeOneForFilter for JdbcReader {
    fn consume_one_for_filter(&self) -> Result<(), DruidError> {
        let mut code_unit = [0_u16; 1];
        assert_eq!(self.read_utf16(&mut code_unit)?, 1);
        Ok(())
    }
}

macro_rules! short_circuit_stream_update_family {
    (
        $index:ident,
        $label:ident,
        $index_with_int_length:ident,
        $label_with_int_length:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $ty:ty
    ) => {
        fn $index(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<$ty>,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            if let Some(value) = value {
                value.consume_one_for_filter()?;
            }
            self.record(format!("{}:{column_index}:{presence}", stringify!($index)));
            Ok(())
        }

        fn $label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<$ty>,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!("{}:{column_label}:{presence}", stringify!($label)));
            Ok(())
        }

        fn $index_with_int_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<$ty>,
            length: i32,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "{}:{column_index}:{presence}:{length}",
                stringify!($index_with_int_length)
            ));
            Ok(())
        }

        fn $label_with_int_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<$ty>,
            length: i32,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "{}:{column_label}:{presence}:{length}",
                stringify!($label_with_int_length)
            ));
            Ok(())
        }

        fn $index_with_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<$ty>,
            length: i64,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "{}:{column_index}:{presence}:{length}",
                stringify!($index_with_length)
            ));
            Ok(())
        }

        fn $label_with_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<$ty>,
            length: i64,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "{}:{column_label}:{presence}:{length}",
                stringify!($label_with_length)
            ));
            Ok(())
        }
    };
}

macro_rules! short_circuit_long_stream_update_family {
    (
        $index:ident,
        $label:ident,
        $index_with_length:ident,
        $label_with_length:ident,
        $ty:ty
    ) => {
        fn $index(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<$ty>,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            if let Some(value) = value {
                value.consume_one_for_filter()?;
            }
            self.record(format!("{}:{column_index}:{presence}", stringify!($index)));
            Ok(())
        }

        fn $label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<$ty>,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!("{}:{column_label}:{presence}", stringify!($label)));
            Ok(())
        }

        fn $index_with_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<$ty>,
            length: i64,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "{}:{column_index}:{presence}:{length}",
                stringify!($index_with_length)
            ));
            Ok(())
        }

        fn $label_with_length(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<$ty>,
            length: i64,
        ) -> Result<(), DruidError> {
            let presence = if value.is_some() { "some" } else { "none" };
            self.record(format!(
                "{}:{column_label}:{presence}:{length}",
                stringify!($label_with_length)
            ));
            if column_label == "fail" {
                Err(DruidError::DriverError(
                    "filtered stream update failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    };
}

impl ResultSetFilter for StreamUpdateShortCircuitFilter {
    short_circuit_stream_update_family!(
        result_set_update_ascii_stream,
        result_set_update_ascii_stream_by_label,
        result_set_update_ascii_stream_with_int_length,
        result_set_update_ascii_stream_by_label_with_int_length,
        result_set_update_ascii_stream_with_length,
        result_set_update_ascii_stream_by_label_with_length,
        JdbcInputStream
    );
    short_circuit_stream_update_family!(
        result_set_update_binary_stream,
        result_set_update_binary_stream_by_label,
        result_set_update_binary_stream_with_int_length,
        result_set_update_binary_stream_by_label_with_int_length,
        result_set_update_binary_stream_with_length,
        result_set_update_binary_stream_by_label_with_length,
        JdbcInputStream
    );
    short_circuit_stream_update_family!(
        result_set_update_character_stream,
        result_set_update_character_stream_by_label,
        result_set_update_character_stream_with_int_length,
        result_set_update_character_stream_by_label_with_int_length,
        result_set_update_character_stream_with_length,
        result_set_update_character_stream_by_label_with_length,
        JdbcReader
    );
    short_circuit_long_stream_update_family!(
        result_set_update_n_character_stream,
        result_set_update_n_character_stream_by_label,
        result_set_update_n_character_stream_with_length,
        result_set_update_n_character_stream_by_label_with_length,
        JdbcReader
    );
}

impl ResultSetFilter for NStringUpdateShortCircuitFilter {
    fn result_set_update_n_string(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        value: Option<String>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "result_set_update_n_string:{column_index}:{value:?}"
            ));
        Ok(())
    }

    fn result_set_update_n_string_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        value: Option<String>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!(
                "result_set_update_n_string_by_label:{column_label}:{value:?}"
            ));
        if column_label == "fail" {
            Err(DruidError::DriverError(
                "filtered NString update failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

impl ResourceUpdateShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

macro_rules! short_circuit_resource_update_pair {
    ($index:ident, $label:ident, $ty:ty) => {
        fn $index(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: Option<$ty>,
        ) -> Result<(), DruidError> {
            self.record(format!("{}:{column_index}:{value:?}", stringify!($index)));
            Ok(())
        }

        fn $label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: Option<$ty>,
        ) -> Result<(), DruidError> {
            self.record(format!("{}:{column_label}:{value:?}", stringify!($label)));
            if column_label == "fail" {
                Err(DruidError::DriverError(
                    "filtered resource update failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    };
}

impl ResultSetFilter for ResourceUpdateShortCircuitFilter {
    short_circuit_resource_update_pair!(
        result_set_update_reference,
        result_set_update_reference_by_label,
        JdbcRef
    );
    short_circuit_resource_update_pair!(
        result_set_update_blob,
        result_set_update_blob_by_label,
        JdbcBlob
    );
    short_circuit_resource_update_pair!(
        result_set_update_clob,
        result_set_update_clob_by_label,
        JdbcClob
    );
    short_circuit_resource_update_pair!(
        result_set_update_array,
        result_set_update_array_by_label,
        JdbcArray
    );
    short_circuit_resource_update_pair!(
        result_set_update_row_id,
        result_set_update_row_id_by_label,
        JdbcRowId
    );
    short_circuit_resource_update_pair!(
        result_set_update_n_clob,
        result_set_update_n_clob_by_label,
        JdbcNClob
    );
    short_circuit_resource_update_pair!(
        result_set_update_sql_xml,
        result_set_update_sql_xml_by_label,
        JdbcSqlXml
    );
}

impl ObjectUpdateShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

impl ResultSetFilter for ObjectUpdateShortCircuitFilter {
    fn result_set_update_object(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        value: JdbcObject,
    ) -> Result<(), DruidError> {
        assert_eq!(value, JdbcObject::Custom(self.expected_custom.clone()));
        let JdbcObject::Custom(custom) = &value else {
            panic!("updateObject 必须保留 vendor custom 对象分支");
        };
        let vendor = custom
            .downcast_ref::<FilterVendorObjectProbe>()
            .expect("必须保留具体 vendor 对象");
        assert_eq!(vendor.id, 99);
        self.record(format!(
            "result_set_update_object:{column_index}:custom:{}:{}",
            custom.class_name(),
            vendor.id
        ));
        Ok(())
    }

    fn result_set_update_object_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        value: JdbcObject,
    ) -> Result<(), DruidError> {
        self.record(format!(
            "result_set_update_object_by_label:{column_label}:{value:?}"
        ));
        if column_label == "fail" {
            Err(DruidError::DriverError(
                "filtered updateObject failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn result_set_update_object_with_scale_or_length(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        value: JdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        self.record(format!(
            "result_set_update_object_with_scale_or_length:{column_index}:{value:?}:{scale_or_length}"
        ));
        Ok(())
    }

    fn result_set_update_object_by_label_with_scale_or_length(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        value: JdbcObject,
        scale_or_length: i32,
    ) -> Result<(), DruidError> {
        self.record(format!(
            "result_set_update_object_by_label_with_scale_or_length:{column_label}:{value:?}:{scale_or_length}"
        ));
        Ok(())
    }
}

#[derive(Debug)]
struct FilterVendorObjectProbe {
    id: u64,
}

impl PhysicalJdbcOpaqueObject for FilterVendorObjectProbe {
    fn class_name(&self) -> &str {
        "example.FilterVendorObject"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ScalarUpdateShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

macro_rules! short_circuit_scalar_update_pair {
    ($index:ident, $label:ident, $ty:ty) => {
        fn $index(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            value: $ty,
        ) -> Result<(), DruidError> {
            self.record(format!("{}:{column_index}:{value:?}", stringify!($index)));
            Ok(())
        }

        fn $label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            value: $ty,
        ) -> Result<(), DruidError> {
            self.record(format!("{}:{column_label}:{value:?}", stringify!($label)));
            if column_label == "fail" {
                Err(DruidError::DriverError(
                    "filtered scalar update failure".to_string(),
                ))
            } else {
                Ok(())
            }
        }
    };
}

impl ResultSetFilter for ScalarUpdateShortCircuitFilter {
    fn result_set_update_null(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<(), DruidError> {
        self.record(format!("result_set_update_null:{column_index}"));
        Ok(())
    }

    fn result_set_update_null_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<(), DruidError> {
        self.record(format!("result_set_update_null_by_label:{column_label}"));
        Ok(())
    }

    short_circuit_scalar_update_pair!(
        result_set_update_boolean,
        result_set_update_boolean_by_label,
        bool
    );
    short_circuit_scalar_update_pair!(result_set_update_byte, result_set_update_byte_by_label, i8);
    short_circuit_scalar_update_pair!(
        result_set_update_short,
        result_set_update_short_by_label,
        i16
    );
    short_circuit_scalar_update_pair!(result_set_update_int, result_set_update_int_by_label, i32);
    short_circuit_scalar_update_pair!(result_set_update_long, result_set_update_long_by_label, i64);
    short_circuit_scalar_update_pair!(
        result_set_update_float,
        result_set_update_float_by_label,
        f32
    );
    short_circuit_scalar_update_pair!(
        result_set_update_double,
        result_set_update_double_by_label,
        f64
    );
    short_circuit_scalar_update_pair!(
        result_set_update_big_decimal,
        result_set_update_big_decimal_by_label,
        Option<BigDecimal>
    );
    short_circuit_scalar_update_pair!(
        result_set_update_string,
        result_set_update_string_by_label,
        Option<String>
    );
    short_circuit_scalar_update_pair!(
        result_set_update_bytes,
        result_set_update_bytes_by_label,
        Option<Vec<u8>>
    );
    short_circuit_scalar_update_pair!(
        result_set_update_date,
        result_set_update_date_by_label,
        Option<NaiveDate>
    );
    short_circuit_scalar_update_pair!(
        result_set_update_time,
        result_set_update_time_by_label,
        Option<NaiveTime>
    );
    short_circuit_scalar_update_pair!(
        result_set_update_timestamp,
        result_set_update_timestamp_by_label,
        Option<NaiveDateTime>
    );
}

impl RowMutationShortCircuitFilter {
    fn record(&self, operation: &str) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(operation.to_string());
    }
}

impl ResultSetFilter for RowMutationShortCircuitFilter {
    fn result_set_insert_row(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("insert_row");
        Ok(())
    }

    fn result_set_update_row(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("update_row");
        Ok(())
    }

    fn result_set_delete_row(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("delete_row");
        Ok(())
    }

    fn result_set_refresh_row(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("refresh_row");
        if self.fail_refresh.load(Ordering::Acquire) {
            Err(DruidError::DriverError(
                "filtered refreshRow failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn result_set_cancel_row_updates(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("cancel_row_updates");
        Ok(())
    }

    fn result_set_move_to_insert_row(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("move_to_insert_row");
        Ok(())
    }

    fn result_set_move_to_current_row(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("move_to_current_row");
        Ok(())
    }
}

struct NStringShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
}

struct MetadataShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
    fail: Arc<AtomicBool>,
}

struct StatementShortCircuitFilter {
    calls: Arc<Mutex<Vec<String>>>,
    replacement: Arc<Mutex<Option<ResultSetStatement>>>,
    fail: Arc<AtomicBool>,
}

impl ResultSetFilter for StatementShortCircuitFilter {
    fn result_set_get_statement(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<ResultSetStatement, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("result_set_get_statement".to_string());
        if self.fail.load(Ordering::Acquire) {
            return Err(DruidError::DriverError(
                "filtered statement failure".to_string(),
            ));
        }
        if let Some(statement) = self
            .replacement
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        {
            return Ok(statement);
        }
        chain.result_set_get_statement()
    }
}

impl ResultSetFilter for MetadataShortCircuitFilter {
    fn result_set_get_meta_data(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
    ) -> Result<ResultSetMetaData, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push("result_set_get_meta_data".to_string());
        if self.fail.load(Ordering::Acquire) {
            Err(DruidError::DriverError(
                "filtered metadata failure".to_string(),
            ))
        } else {
            Ok(ResultSetMetaData::new(Vec::new()))
        }
    }
}

impl ResultSetFilter for NStringShortCircuitFilter {
    fn result_set_get_n_string(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Option<String>, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("result_set_get_n_string:{column_index}"));
        Ok(Some("过滤-索引".to_string()))
    }

    fn result_set_get_n_string_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Option<String>, DruidError> {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("result_set_get_n_string_by_label:{column_label}"));
        if column_label == "fail" {
            Err(DruidError::DriverError(
                "filtered NString failure".to_string(),
            ))
        } else {
            Ok(Some("过滤-标签".to_string()))
        }
    }
}

impl NavigationShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

macro_rules! short_circuit_no_arg_filter_method {
    ($method:ident, $ty:ty, $value:expr) => {
        fn $method(&self, _chain: &mut ResultSetFilterChain<'_>) -> Result<$ty, DruidError> {
            self.record(stringify!($method).to_string());
            Ok($value)
        }
    };
}

macro_rules! short_circuit_i32_arg_filter_method {
    ($method:ident, $argument:ident, $ty:ty, $value:expr) => {
        fn $method(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            $argument: i32,
        ) -> Result<$ty, DruidError> {
            self.record(format!("{}:{}", stringify!($method), $argument));
            Ok($value)
        }
    };
}

impl ResultSetFilter for NavigationShortCircuitFilter {
    short_circuit_no_arg_filter_method!(result_set_was_null, bool, false);
    short_circuit_no_arg_filter_method!(result_set_previous, bool, true);
    short_circuit_no_arg_filter_method!(result_set_is_before_first, bool, false);
    short_circuit_no_arg_filter_method!(result_set_is_after_last, bool, true);
    short_circuit_no_arg_filter_method!(result_set_is_first, bool, true);
    short_circuit_no_arg_filter_method!(result_set_is_last, bool, false);
    short_circuit_no_arg_filter_method!(result_set_before_first, (), ());
    short_circuit_no_arg_filter_method!(result_set_after_last, (), ());
    short_circuit_no_arg_filter_method!(result_set_first, bool, false);
    short_circuit_no_arg_filter_method!(result_set_last, bool, false);
    short_circuit_no_arg_filter_method!(result_set_get_row, i32, 99);
    short_circuit_no_arg_filter_method!(result_set_get_fetch_direction, i32, 2000);
    short_circuit_no_arg_filter_method!(result_set_get_fetch_size, i32, 128);
    short_circuit_no_arg_filter_method!(result_set_get_type, i32, 2004);
    short_circuit_no_arg_filter_method!(result_set_get_concurrency, i32, 2007);
    short_circuit_no_arg_filter_method!(result_set_get_holdability, i32, 2);
    short_circuit_no_arg_filter_method!(
        result_set_get_cursor_name,
        Option<String>,
        Some("filtered-cursor".to_string())
    );
    short_circuit_no_arg_filter_method!(result_set_row_updated, bool, false);
    short_circuit_no_arg_filter_method!(result_set_row_inserted, bool, true);
    short_circuit_no_arg_filter_method!(result_set_row_deleted, bool, false);
    short_circuit_no_arg_filter_method!(result_set_is_closed, bool, false);
    short_circuit_i32_arg_filter_method!(result_set_absolute, row, bool, row == 50);
    short_circuit_i32_arg_filter_method!(result_set_relative, rows, bool, rows == -20);
    short_circuit_i32_arg_filter_method!(result_set_set_fetch_direction, direction, (), ());
    short_circuit_i32_arg_filter_method!(result_set_set_fetch_size, rows, (), ());

    fn result_set_find_column(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<usize, DruidError> {
        self.record(format!("result_set_find_column:{column_label}"));
        if column_label == "fail" {
            Err(DruidError::DriverError(
                "filtered findColumn failure".to_string(),
            ))
        } else {
            Ok(88)
        }
    }
}

impl ResourceShortCircuitFilter {
    fn record_index(&self, operation: &str, column_index: usize) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{operation}:{column_index}"));
    }

    fn record_label(&self, operation: &str, column_label: &str) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{operation}:{column_label}"));
    }
}

macro_rules! short_circuit_resource_filter_pair {
    ($index:ident, $label:ident, $ty:ty) => {
        fn $index(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
        ) -> Result<Option<$ty>, DruidError> {
            self.record_index(stringify!($index), column_index);
            Ok(None)
        }

        fn $label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
        ) -> Result<Option<$ty>, DruidError> {
            self.record_label(stringify!($label), column_label);
            Ok(None)
        }
    };
}

impl ResultSetFilter for ResourceShortCircuitFilter {
    short_circuit_resource_filter_pair!(result_set_get_ref, result_set_get_ref_by_label, JdbcRef);
    short_circuit_resource_filter_pair!(
        result_set_get_blob,
        result_set_get_blob_by_label,
        JdbcBlob
    );
    short_circuit_resource_filter_pair!(
        result_set_get_clob,
        result_set_get_clob_by_label,
        JdbcClob
    );
    short_circuit_resource_filter_pair!(
        result_set_get_array,
        result_set_get_array_by_label,
        JdbcArray
    );
    short_circuit_resource_filter_pair!(result_set_get_url, result_set_get_url_by_label, JdbcUrl);
    short_circuit_resource_filter_pair!(
        result_set_get_row_id,
        result_set_get_row_id_by_label,
        JdbcRowId
    );
    short_circuit_resource_filter_pair!(
        result_set_get_n_clob,
        result_set_get_n_clob_by_label,
        JdbcNClob
    );

    fn result_set_get_sql_xml(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Option<JdbcSqlXml>, DruidError> {
        self.record_index("result_set_get_sql_xml", column_index);
        Ok(None)
    }

    fn result_set_get_sql_xml_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Option<JdbcSqlXml>, DruidError> {
        self.record_label("result_set_get_sql_xml_by_label", column_label);
        if column_label == "fail" {
            Err(DruidError::DriverError(
                "filtered SQLXML failure".to_string(),
            ))
        } else {
            Ok(None)
        }
    }

    fn result_set_get_ascii_stream(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.record_index("result_set_get_ascii_stream", column_index);
        Ok(Some(self.stream.clone()))
    }

    fn result_set_get_ascii_stream_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Option<JdbcInputStream>, DruidError> {
        self.record_label("result_set_get_ascii_stream_by_label", column_label);
        Ok(Some(self.stream.clone()))
    }

    short_circuit_resource_filter_pair!(
        result_set_get_unicode_stream,
        result_set_get_unicode_stream_by_label,
        JdbcInputStream
    );
    short_circuit_resource_filter_pair!(
        result_set_get_binary_stream,
        result_set_get_binary_stream_by_label,
        JdbcInputStream
    );

    fn result_set_get_character_stream(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Option<JdbcReader>, DruidError> {
        self.record_index("result_set_get_character_stream", column_index);
        Ok(Some(self.reader.clone()))
    }

    fn result_set_get_character_stream_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        self.record_label("result_set_get_character_stream_by_label", column_label);
        Ok(Some(self.reader.clone()))
    }

    short_circuit_resource_filter_pair!(
        result_set_get_n_character_stream,
        result_set_get_n_character_stream_by_label,
        JdbcReader
    );
}

impl ObjectShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

impl ResultSetFilter for ObjectShortCircuitFilter {
    fn result_set_get_object(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        self.record(format!("result_set_get_object:{column_index}"));
        Ok(Value::Int(901))
    }

    fn result_set_get_object_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        self.record(format!("result_set_get_object_by_label:{column_label}"));
        Ok(Value::String("filtered-label-object".to_string()))
    }

    fn result_set_get_object_with_type_map(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        let identity = type_map_identity(type_map);
        self.record(format!(
            "result_set_get_object_with_type_map:{column_index}:{identity}"
        ));
        Ok(JdbcObject::String(format!("filtered-map:{identity}")))
    }

    fn result_set_get_object_by_label_with_type_map(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        let identity = type_map_identity(type_map);
        self.record(format!(
            "result_set_get_object_by_label_with_type_map:{column_label}:{identity}"
        ));
        Ok(JdbcObject::String(format!("filtered-label-map:{identity}")))
    }

    fn result_set_get_object_typed(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        self.record(format!(
            "result_set_get_object_typed:{column_index}:{target_type:?}"
        ));
        Ok(JdbcObject::String(format!(
            "filtered-typed:{target_type:?}"
        )))
    }

    fn result_set_get_object_typed_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        self.record(format!(
            "result_set_get_object_typed_by_label:{column_label}:{target_type:?}"
        ));
        if column_label == "fail" {
            Err(DruidError::DriverError(
                "filtered object failure".to_string(),
            ))
        } else {
            Ok(JdbcObject::String(format!(
                "filtered-label-typed:{target_type:?}"
            )))
        }
    }
}

impl StrongGetterShortCircuitFilter {
    fn record(&self, call: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(call);
    }
}

macro_rules! short_circuit_temporal_filter_family {
    (
        $index:ident, $label:ident, $index_calendar:ident, $label_calendar:ident,
        $ty:ty, $index_value:expr, $label_value:expr, $calendar_value:expr
    ) => {
        fn $index(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
        ) -> Result<Option<$ty>, DruidError> {
            self.record(format!("{}:{column_index}", stringify!($index)));
            Ok(Some($index_value))
        }

        fn $label(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
        ) -> Result<Option<$ty>, DruidError> {
            self.record(format!("{}:{column_label}", stringify!($label)));
            Ok(Some($label_value))
        }

        fn $index_calendar(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_index: usize,
            calendar: &JdbcCalendarArgument,
        ) -> Result<Option<$ty>, DruidError> {
            self.record(format!(
                "{}:{column_index}:{}",
                stringify!($index_calendar),
                calendar_identity(calendar)
            ));
            Ok(Some($calendar_value))
        }

        fn $label_calendar(
            &self,
            _chain: &mut ResultSetFilterChain<'_>,
            column_label: &str,
            calendar: &JdbcCalendarArgument,
        ) -> Result<Option<$ty>, DruidError> {
            self.record(format!(
                "{}:{column_label}:{}",
                stringify!($label_calendar),
                calendar_identity(calendar)
            ));
            if column_label == "fail" {
                Err(DruidError::DriverError(
                    "filtered temporal failure".to_string(),
                ))
            } else {
                Ok(Some($calendar_value))
            }
        }
    };
}

impl ResultSetFilter for StrongGetterShortCircuitFilter {
    fn result_set_get_big_decimal(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.record(format!("result_set_get_big_decimal:{column_index}"));
        Ok(Some(BigDecimal::from(101)))
    }

    fn result_set_get_big_decimal_by_label(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.record(format!(
            "result_set_get_big_decimal_by_label:{column_label}"
        ));
        Ok(Some(BigDecimal::from(102)))
    }

    fn result_set_get_big_decimal_with_scale(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.record(format!(
            "result_set_get_big_decimal_with_scale:{column_index}:{scale}"
        ));
        Ok(Some(BigDecimal::from(103)))
    }

    fn result_set_get_big_decimal_by_label_with_scale(
        &self,
        _chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.record(format!(
            "result_set_get_big_decimal_by_label_with_scale:{column_label}:{scale}"
        ));
        Ok(Some(BigDecimal::from(104)))
    }

    short_circuit_temporal_filter_family!(
        result_set_get_date,
        result_set_get_date_by_label,
        result_set_get_date_with_calendar,
        result_set_get_date_by_label_with_calendar,
        NaiveDate,
        NaiveDate::from_ymd_opt(2031, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2031, 1, 2).unwrap(),
        NaiveDate::from_ymd_opt(2031, 1, 3).unwrap()
    );
    short_circuit_temporal_filter_family!(
        result_set_get_time,
        result_set_get_time_by_label,
        result_set_get_time_with_calendar,
        result_set_get_time_by_label_with_calendar,
        NaiveTime,
        NaiveTime::from_hms_opt(1, 2, 3).unwrap(),
        NaiveTime::from_hms_opt(2, 3, 4).unwrap(),
        NaiveTime::from_hms_opt(3, 4, 5).unwrap()
    );
    short_circuit_temporal_filter_family!(
        result_set_get_timestamp,
        result_set_get_timestamp_by_label,
        result_set_get_timestamp_with_calendar,
        result_set_get_timestamp_by_label_with_calendar,
        NaiveDateTime,
        NaiveDate::from_ymd_opt(2031, 1, 1)
            .unwrap()
            .and_hms_opt(1, 2, 3)
            .unwrap(),
        NaiveDate::from_ymd_opt(2031, 1, 2)
            .unwrap()
            .and_hms_opt(2, 3, 4)
            .unwrap(),
        NaiveDate::from_ymd_opt(2031, 1, 3)
            .unwrap()
            .and_hms_opt(3, 4, 5)
            .unwrap()
    );
}

impl OrderingResultSetFilter {
    fn record(&self, operation: &str) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}:{operation}", self.label));
    }
}

impl ResultSetFilter for OrderingResultSetFilter {
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.record("open");
        Ok(())
    }

    fn result_set_next(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<bool, DruidError> {
        self.record("next-before");
        if self.short_circuit_next {
            self.record("next-short");
            return Ok(false);
        }
        let result = chain.result_set_next();
        if result.is_ok() {
            self.record("next-after");
        }
        result
    }

    fn result_set_close(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<(), DruidError> {
        self.record("close-before");
        let result = chain.result_set_close();
        if result.is_ok() {
            self.record("close-after");
        }
        result
    }
}

fn calls(calls: &Mutex<Vec<String>>) -> Vec<String> {
    calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[test]
fn result_set_filter_chain_preserves_java_order_short_circuit_and_error_unwind() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&call_log), true);
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(OrderingResultSetFilter {
        label: "first",
        calls: Arc::clone(&call_log),
        short_circuit_next: false,
    }));
    chain.add_result_set(Arc::new(OrderingResultSetFilter {
        label: "second",
        calls: Arc::clone(&call_log),
        short_circuit_next: false,
    }));

    assert_eq!(chain.result_set_count(), 2);
    chain.result_set_open_after(&context).unwrap();
    assert!(chain.result_set_next(&physical, &context).unwrap());
    assert_eq!(
        calls(&call_log),
        vec![
            "second:open",
            "first:open",
            "first:next-before",
            "second:next-before",
            "physical:next",
            "second:next-after",
            "first:next-after",
        ]
    );

    assert_eq!(
        chain.result_set_close(&physical, &context),
        Err(DruidError::DriverError("close failed".to_string()))
    );
    assert_eq!(
        &calls(&call_log)[7..],
        [
            "first:close-before",
            "second:close-before",
            "physical:close"
        ]
    );

    let short_log = Arc::new(Mutex::new(Vec::new()));
    let short_physical = PhysicalResultSetProbe::new(Arc::clone(&short_log), false);
    let short_context = ResultSetFilterContext::new();
    let mut short_chain = FilterChain::new();
    short_chain.add_result_set(Arc::new(OrderingResultSetFilter {
        label: "short",
        calls: Arc::clone(&short_log),
        short_circuit_next: true,
    }));
    assert!(!short_chain
        .result_set_next(&short_physical, &short_context)
        .unwrap());
    assert_eq!(calls(&short_log), ["short:next-before", "short:next-short"]);
    assert_eq!(short_physical.next_count.load(Ordering::Acquire), 0);

    let scalar_log = Arc::new(Mutex::new(Vec::new()));
    let scalar_physical = PhysicalResultSetProbe::new(Arc::clone(&scalar_log), false);
    let scalar_context = ResultSetFilterContext::new();
    let mut scalar_chain = FilterChain::new();
    scalar_chain.add_result_set(Arc::new(IntWrappingFilter {
        label: "first",
        delta: 1,
        calls: Arc::clone(&scalar_log),
    }));
    scalar_chain.add_result_set(Arc::new(IntWrappingFilter {
        label: "second",
        delta: 10,
        calls: Arc::clone(&scalar_log),
    }));
    assert_eq!(
        scalar_chain
            .result_set_get_int(&scalar_physical, &scalar_context, 5)
            .unwrap(),
        43
    );
    assert_eq!(
        calls(&scalar_log),
        [
            "first:int-before:5",
            "second:int-before:5",
            "physical:int:5",
            "second:int-after:32",
            "first:int-after:42",
        ]
    );
}

#[test]
fn result_set_filter_defaults_delegate_and_context_defaults_are_observable() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(call_log, false);
    let context = ResultSetFilterContext::default();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(PassThroughResultSetFilter));

    assert_eq!(context.elapsed(), None);
    assert_eq!(context.fetch_row_count(), 0);
    assert_eq!(context.close_count(), 0);
    chain.result_set_open_after(&context).unwrap();
    assert!(chain.result_set_next(&physical, &context).unwrap());
    assert_eq!(
        chain.result_set_get_string(&physical, &context, 1).unwrap(),
        Some("index-string".to_string())
    );
    assert_eq!(
        chain
            .result_set_get_string_by_label(&physical, &context, "string")
            .unwrap(),
        Some("label-string".to_string())
    );
    assert!(chain
        .result_set_get_boolean(&physical, &context, 2)
        .unwrap());
    assert!(!chain
        .result_set_get_boolean_by_label(&physical, &context, "boolean")
        .unwrap());
    assert_eq!(
        chain.result_set_get_byte(&physical, &context, 3).unwrap(),
        8
    );
    assert_eq!(
        chain
            .result_set_get_byte_by_label(&physical, &context, "byte")
            .unwrap(),
        9
    );
    assert_eq!(
        chain.result_set_get_short(&physical, &context, 4).unwrap(),
        16
    );
    assert_eq!(
        chain
            .result_set_get_short_by_label(&physical, &context, "short")
            .unwrap(),
        17
    );
    assert_eq!(
        chain.result_set_get_int(&physical, &context, 5).unwrap(),
        32
    );
    assert_eq!(
        chain
            .result_set_get_int_by_label(&physical, &context, "int")
            .unwrap(),
        33
    );
    assert_eq!(
        chain.result_set_get_long(&physical, &context, 6).unwrap(),
        64
    );
    assert_eq!(
        chain
            .result_set_get_long_by_label(&physical, &context, "long")
            .unwrap(),
        65
    );
    assert_eq!(
        chain.result_set_get_float(&physical, &context, 7).unwrap(),
        1.25
    );
    assert_eq!(
        chain
            .result_set_get_float_by_label(&physical, &context, "float")
            .unwrap(),
        2.25
    );
    assert_eq!(
        chain.result_set_get_double(&physical, &context, 8).unwrap(),
        3.5
    );
    assert_eq!(
        chain
            .result_set_get_double_by_label(&physical, &context, "double")
            .unwrap(),
        4.5
    );
    assert_eq!(
        chain.result_set_get_bytes(&physical, &context, 9).unwrap(),
        Some(vec![1, 2])
    );
    assert_eq!(
        chain
            .result_set_get_bytes_by_label(&physical, &context, "bytes")
            .unwrap(),
        Some(vec![3, 4])
    );
    assert_eq!(
        chain
            .result_set_get_big_decimal(&physical, &context, 10)
            .unwrap(),
        Some(BigDecimal::from_str("12.340").unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_big_decimal_by_label(&physical, &context, "decimal")
            .unwrap(),
        Some(BigDecimal::from_str("56.780").unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_big_decimal_with_scale(&physical, &context, 11, 2)
            .unwrap(),
        Some(BigDecimal::from_str("12.340").unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_big_decimal_by_label_with_scale(
                &physical,
                &context,
                "decimal_scale",
                3,
            )
            .unwrap(),
        Some(BigDecimal::from_str("56.780").unwrap())
    );
    let null_calendar = JdbcCalendarArgument::specified(None);
    let shanghai_calendar =
        JdbcCalendarArgument::specified(Some(JdbcCalendar::new("Asia/Shanghai").unwrap()));
    assert_eq!(
        chain.result_set_get_date(&physical, &context, 12).unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_date_by_label(&physical, &context, "date")
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 2, 3).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_date_with_calendar(&physical, &context, 13, &null_calendar)
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_date_by_label_with_calendar(
                &physical,
                &context,
                "date_calendar",
                &shanghai_calendar,
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 2, 3).unwrap())
    );
    assert_eq!(
        chain.result_set_get_time(&physical, &context, 14).unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_time_by_label(&physical, &context, "time")
            .unwrap(),
        Some(NaiveTime::from_hms_opt(6, 7, 8).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_time_with_calendar(&physical, &context, 15, &null_calendar)
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_time_by_label_with_calendar(
                &physical,
                &context,
                "time_calendar",
                &shanghai_calendar,
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(6, 7, 8).unwrap())
    );
    assert_eq!(
        chain
            .result_set_get_timestamp(&physical, &context, 16)
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(3, 4, 5)
                .unwrap()
        )
    );
    assert_eq!(
        chain
            .result_set_get_timestamp_by_label(&physical, &context, "timestamp")
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 2, 3)
                .unwrap()
                .and_hms_opt(6, 7, 8)
                .unwrap()
        )
    );
    assert_eq!(
        chain
            .result_set_get_timestamp_with_calendar(&physical, &context, 17, &null_calendar,)
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(3, 4, 5)
                .unwrap()
        )
    );
    assert_eq!(
        chain
            .result_set_get_timestamp_by_label_with_calendar(
                &physical,
                &context,
                "timestamp_calendar",
                &shanghai_calendar,
            )
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 2, 3)
                .unwrap()
                .and_hms_opt(6, 7, 8)
                .unwrap()
        )
    );
    let mut type_map = JdbcTypeMap::new();
    type_map.insert(
        "example.address",
        JdbcTargetType::Custom("example.Address".to_string()),
    );
    assert_eq!(
        chain
            .result_set_get_object(&physical, &context, 18)
            .unwrap(),
        Value::Int(18)
    );
    assert_eq!(
        chain
            .result_set_get_object_by_label(&physical, &context, "object")
            .unwrap(),
        Value::String("object".to_string())
    );
    assert_eq!(
        chain
            .result_set_get_object_with_type_map(&physical, &context, 19, None)
            .unwrap(),
        JdbcObject::String("index-map:19:null".to_string())
    );
    assert_eq!(
        chain
            .result_set_get_object_by_label_with_type_map(
                &physical,
                &context,
                "mapped",
                Some(&type_map),
            )
            .unwrap(),
        JdbcObject::String(
            "label-map:mapped:example.address=Custom(\"example.Address\")".to_string()
        )
    );
    assert_eq!(
        chain
            .result_set_get_object_typed(
                &physical,
                &context,
                20,
                &JdbcTargetType::Custom("example.Typed".to_string()),
            )
            .unwrap(),
        JdbcObject::String("index-typed:20:Custom(\"example.Typed\")".to_string())
    );
    assert_eq!(
        chain
            .result_set_get_object_typed_by_label(
                &physical,
                &context,
                "typed",
                &JdbcTargetType::String,
            )
            .unwrap(),
        JdbcObject::String("label-typed:typed:String".to_string())
    );
    assert_eq!(
        chain.result_set_warnings(&physical, &context),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_warnings"
        })
    );
    assert_eq!(
        chain.result_set_clear_warnings(&physical, &context),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_clear_warnings"
        })
    );
    chain.result_set_close(&physical, &context).unwrap();
    context.increment_close_count();
    assert_eq!(context.close_count(), 1);
    assert_eq!(
        calls(&physical.calls)[1..19],
        [
            "physical:string:1",
            "physical:string_by_label:string",
            "physical:boolean:2",
            "physical:boolean_by_label:boolean",
            "physical:byte:3",
            "physical:byte_by_label:byte",
            "physical:short:4",
            "physical:short_by_label:short",
            "physical:int:5",
            "physical:int_by_label:int",
            "physical:long:6",
            "physical:long_by_label:long",
            "physical:float:7",
            "physical:float_by_label:float",
            "physical:double:8",
            "physical:double_by_label:double",
            "physical:bytes:9",
            "physical:bytes_by_label:bytes",
        ]
    );
    assert_eq!(
        calls(&physical.calls)[19..35],
        [
            "physical:big_decimal:10:None",
            "physical:big_decimal_by_label:decimal:None",
            "physical:big_decimal:11:Some(2)",
            "physical:big_decimal_by_label:decimal_scale:Some(3)",
            "physical:date:12:unspecified",
            "physical:date_by_label:date:unspecified",
            "physical:date:13:specified:null",
            "physical:date_by_label:date_calendar:specified:Asia/Shanghai",
            "physical:time:14:unspecified",
            "physical:time_by_label:time:unspecified",
            "physical:time:15:specified:null",
            "physical:time_by_label:time_calendar:specified:Asia/Shanghai",
            "physical:timestamp:16:unspecified",
            "physical:timestamp_by_label:timestamp:unspecified",
            "physical:timestamp:17:specified:null",
            "physical:timestamp_by_label:timestamp_calendar:specified:Asia/Shanghai",
        ]
    );
    assert_eq!(
        calls(&physical.calls)[35..41],
        [
            "physical:value:18",
            "physical:value_by_label:object",
            "physical:object_with_type_map:19:null",
            "physical:object_by_label_with_type_map:mapped:example.address=Custom(\"example.Address\")",
            "physical:object_as:20:Custom(\"example.Typed\")",
            "physical:object_by_label_as:typed:String",
        ]
    );
}

#[test]
fn result_set_resource_filter_defaults_preserve_all_physical_overload_identities() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&call_log), false);
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(PassThroughResultSetFilter));

    macro_rules! assert_resource_pair {
        ($index:ident, $label:ident, $position:expr, $name:literal) => {
            assert!(chain
                .$index(&physical, &context, $position)
                .unwrap()
                .is_none());
            assert!(chain.$label(&physical, &context, $name).unwrap().is_none());
        };
    }

    assert_resource_pair!(result_set_get_ref, result_set_get_ref_by_label, 1, "ref");
    assert_resource_pair!(result_set_get_blob, result_set_get_blob_by_label, 2, "blob");
    assert_resource_pair!(result_set_get_clob, result_set_get_clob_by_label, 3, "clob");
    assert_resource_pair!(
        result_set_get_array,
        result_set_get_array_by_label,
        4,
        "array"
    );
    assert_resource_pair!(result_set_get_url, result_set_get_url_by_label, 5, "url");
    assert_resource_pair!(
        result_set_get_row_id,
        result_set_get_row_id_by_label,
        6,
        "row_id"
    );
    assert_resource_pair!(
        result_set_get_n_clob,
        result_set_get_n_clob_by_label,
        7,
        "n_clob"
    );
    assert_resource_pair!(
        result_set_get_sql_xml,
        result_set_get_sql_xml_by_label,
        8,
        "sql_xml"
    );
    assert_resource_pair!(
        result_set_get_ascii_stream,
        result_set_get_ascii_stream_by_label,
        9,
        "ascii"
    );
    assert_resource_pair!(
        result_set_get_unicode_stream,
        result_set_get_unicode_stream_by_label,
        10,
        "unicode"
    );
    assert_resource_pair!(
        result_set_get_binary_stream,
        result_set_get_binary_stream_by_label,
        11,
        "binary"
    );
    assert_resource_pair!(
        result_set_get_character_stream,
        result_set_get_character_stream_by_label,
        12,
        "character"
    );
    assert_resource_pair!(
        result_set_get_n_character_stream,
        result_set_get_n_character_stream_by_label,
        13,
        "n_character"
    );

    assert_eq!(
        calls(&call_log),
        [
            "physical:reference:1",
            "physical:reference_by_label:ref",
            "physical:blob:2",
            "physical:blob_by_label:blob",
            "physical:clob:3",
            "physical:clob_by_label:clob",
            "physical:array:4",
            "physical:array_by_label:array",
            "physical:url:5",
            "physical:url_by_label:url",
            "physical:row_id:6",
            "physical:row_id_by_label:row_id",
            "physical:n_clob:7",
            "physical:n_clob_by_label:n_clob",
            "physical:sql_xml:8",
            "physical:sql_xml_by_label:sql_xml",
            "physical:ascii_stream:9",
            "physical:ascii_stream_by_label:ascii",
            "physical:unicode_stream:10",
            "physical:unicode_stream_by_label:unicode",
            "physical:binary_stream:11",
            "physical:binary_stream_by_label:binary",
            "physical:character_stream:12",
            "physical:character_stream_by_label:character",
            "physical:n_character_stream:13",
            "physical:n_character_stream_by_label:n_character",
        ]
    );
}

#[test]
fn result_set_navigation_filter_defaults_delegate_every_physical_method() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&call_log), false);
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(PassThroughResultSetFilter));

    assert!(chain.result_set_was_null(&physical, &context).unwrap());
    assert!(!chain.result_set_previous(&physical, &context).unwrap());
    assert!(chain
        .result_set_is_before_first(&physical, &context)
        .unwrap());
    assert!(!chain.result_set_is_after_last(&physical, &context).unwrap());
    assert!(!chain.result_set_is_first(&physical, &context).unwrap());
    assert!(chain.result_set_is_last(&physical, &context).unwrap());
    chain.result_set_before_first(&physical, &context).unwrap();
    chain.result_set_after_last(&physical, &context).unwrap();
    assert!(chain.result_set_first(&physical, &context).unwrap());
    assert!(chain.result_set_last(&physical, &context).unwrap());
    assert_eq!(chain.result_set_get_row(&physical, &context).unwrap(), 7);
    assert!(chain.result_set_absolute(&physical, &context, 5).unwrap());
    assert!(chain.result_set_relative(&physical, &context, -2).unwrap());
    chain
        .result_set_set_fetch_direction(&physical, &context, 1000)
        .unwrap();
    assert_eq!(
        chain
            .result_set_get_fetch_direction(&physical, &context)
            .unwrap(),
        1000
    );
    chain
        .result_set_set_fetch_size(&physical, &context, 64)
        .unwrap();
    assert_eq!(
        chain
            .result_set_get_fetch_size(&physical, &context)
            .unwrap(),
        64
    );
    assert_eq!(
        chain.result_set_get_type(&physical, &context).unwrap(),
        1004
    );
    assert_eq!(
        chain
            .result_set_get_concurrency(&physical, &context)
            .unwrap(),
        1007
    );
    assert_eq!(
        chain
            .result_set_get_holdability(&physical, &context)
            .unwrap(),
        1
    );
    assert_eq!(
        chain
            .result_set_get_cursor_name(&physical, &context)
            .unwrap(),
        Some("cursor".to_string())
    );
    assert!(chain.result_set_row_updated(&physical, &context).unwrap());
    assert!(!chain.result_set_row_inserted(&physical, &context).unwrap());
    assert!(chain.result_set_row_deleted(&physical, &context).unwrap());
    assert_eq!(
        chain
            .result_set_find_column(&physical, &context, "named")
            .unwrap(),
        23
    );
    assert!(!chain.result_set_is_closed(&physical, &context).unwrap());

    let observed = calls(&call_log);
    assert_eq!(observed.len(), 25);
    for expected in [
        "physical:was_null",
        "physical:previous",
        "physical:is_before_first",
        "physical:is_after_last",
        "physical:is_first",
        "physical:is_last",
        "physical:before_first",
        "physical:after_last",
        "physical:first",
        "physical:last",
        "physical:row",
        "physical:absolute:5",
        "physical:relative:-2",
        "physical:set_fetch_direction:1000",
        "physical:fetch_direction",
        "physical:set_fetch_size:64",
        "physical:fetch_size",
        "physical:result_set_type",
        "physical:concurrency",
        "physical:holdability",
        "physical:cursor_name",
        "physical:row_updated",
        "physical:row_inserted",
        "physical:row_deleted",
        "physical:find_column:named",
    ] {
        assert!(observed.iter().any(|call| call == expected), "{expected}");
    }
}

#[test]
fn result_set_row_mutation_filter_defaults_delegate_every_physical_method() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&call_log), false);
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(PassThroughResultSetFilter));

    // SOURCE_PARITY / V2_MIRRORED：七个 Java 无参方法均从 position 0
    // 进入默认 Filter，并在末端调用同名 raw ResultSet 方法。
    chain.result_set_insert_row(&physical, &context).unwrap();
    chain.result_set_update_row(&physical, &context).unwrap();
    chain.result_set_delete_row(&physical, &context).unwrap();
    chain.result_set_refresh_row(&physical, &context).unwrap();
    chain
        .result_set_cancel_row_updates(&physical, &context)
        .unwrap();
    chain
        .result_set_move_to_insert_row(&physical, &context)
        .unwrap();
    chain
        .result_set_move_to_current_row(&physical, &context)
        .unwrap();

    assert_eq!(
        calls(&call_log),
        [
            "physical:insert_row",
            "physical:update_row",
            "physical:delete_row",
            "physical:refresh_row",
            "physical:cancel_row_updates",
            "physical:move_to_insert_row",
            "physical:move_to_current_row",
        ]
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn result_set_scalar_update_filter_defaults_delegate_all_java_overloads() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&call_log), false);
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let decimal = BigDecimal::from_str("12.340").unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_opt(10, 11, 12).unwrap();
    let timestamp = date.and_time(time);

    // SOURCE_PARITY / V2_MIRRORED：Java Filter 的 14 个 setter ×
    // 下标/标签重载均从 position 0 进入独立入口，末端才编码为物理 SPI 描述。
    chain
        .result_set_update_null(&physical, &context, 1)
        .unwrap();
    chain
        .result_set_update_null_by_label(&physical, &context, "null_value")
        .unwrap();
    chain
        .result_set_update_boolean(&physical, &context, 2, true)
        .unwrap();
    chain
        .result_set_update_boolean_by_label(&physical, &context, "boolean_value", false)
        .unwrap();
    chain
        .result_set_update_byte(&physical, &context, 3, -8)
        .unwrap();
    chain
        .result_set_update_byte_by_label(&physical, &context, "byte_value", 8)
        .unwrap();
    chain
        .result_set_update_short(&physical, &context, 4, -16)
        .unwrap();
    chain
        .result_set_update_short_by_label(&physical, &context, "short_value", 16)
        .unwrap();
    chain
        .result_set_update_int(&physical, &context, 5, -32)
        .unwrap();
    chain
        .result_set_update_int_by_label(&physical, &context, "int_value", 32)
        .unwrap();
    chain
        .result_set_update_long(&physical, &context, 6, -64)
        .unwrap();
    chain
        .result_set_update_long_by_label(&physical, &context, "long_value", 64)
        .unwrap();
    chain
        .result_set_update_float(&physical, &context, 7, 1.25)
        .unwrap();
    chain
        .result_set_update_float_by_label(&physical, &context, "float_value", -1.25)
        .unwrap();
    chain
        .result_set_update_double(&physical, &context, 8, 2.5)
        .unwrap();
    chain
        .result_set_update_double_by_label(&physical, &context, "double_value", -2.5)
        .unwrap();
    chain
        .result_set_update_big_decimal(&physical, &context, 9, Some(decimal.clone()))
        .unwrap();
    chain
        .result_set_update_big_decimal_by_label(&physical, &context, "decimal_value", None)
        .unwrap();
    chain
        .result_set_update_string(&physical, &context, 10, Some("index".to_string()))
        .unwrap();
    chain
        .result_set_update_string_by_label(&physical, &context, "string_value", None)
        .unwrap();
    chain
        .result_set_update_bytes(&physical, &context, 11, Some(vec![0, 255]))
        .unwrap();
    chain
        .result_set_update_bytes_by_label(&physical, &context, "bytes_value", None)
        .unwrap();
    chain
        .result_set_update_date(&physical, &context, 12, Some(date))
        .unwrap();
    chain
        .result_set_update_date_by_label(&physical, &context, "date_value", None)
        .unwrap();
    chain
        .result_set_update_time(&physical, &context, 13, Some(time))
        .unwrap();
    chain
        .result_set_update_time_by_label(&physical, &context, "time_value", None)
        .unwrap();
    chain
        .result_set_update_timestamp(&physical, &context, 14, Some(timestamp))
        .unwrap();
    chain
        .result_set_update_timestamp_by_label(&physical, &context, "timestamp_value", None)
        .unwrap();

    let observed = calls(&call_log);
    assert_eq!(observed.len(), 28);
    assert_eq!(observed[0], "physical:update_value:1:Null");
    assert_eq!(
        observed[1],
        "physical:update_value_by_label:null_value:Null"
    );
    assert_eq!(observed[2], "physical:update_value:2:Boolean(true)");
    assert_eq!(
        observed[3],
        "physical:update_value_by_label:boolean_value:Boolean(false)"
    );
    assert_eq!(
        observed[16],
        "physical:update_value:9:BigDecimal(Some(BigDecimal(sign=Plus, scale=3, digits=[12340])))"
    );
    assert_eq!(
        observed[17],
        "physical:update_value_by_label:decimal_value:BigDecimal(None)"
    );
    assert_eq!(
        observed[20],
        "physical:update_value:11:Bytes(Some([0, 255]))"
    );
    assert_eq!(
        observed[27],
        "physical:update_value_by_label:timestamp_value:Timestamp(None)"
    );
}

#[test]
fn result_set_object_update_filter_defaults_preserve_all_four_overloads() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&call_log), false);
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(PassThroughResultSetFilter));

    // SOURCE_PARITY / V2_MIRRORED：四个 Java updateObject 重载必须分别
    // 从 position 0 进入默认 Filter，末端保持 Object 类型与 scaleOrLength。
    chain
        .result_set_update_object(
            &physical,
            &context,
            15,
            JdbcObject::String("index".to_string()),
        )
        .unwrap();
    chain
        .result_set_update_object_by_label(&physical, &context, "object_value", Value::Null.into())
        .unwrap();
    chain
        .result_set_update_object_with_scale_or_length(
            &physical,
            &context,
            16,
            JdbcObject::Integer(7),
            -3,
        )
        .unwrap();
    chain
        .result_set_update_object_by_label_with_scale_or_length(
            &physical,
            &context,
            "scaled_object",
            JdbcObject::Bytes(vec![9]),
            99,
        )
        .unwrap();

    assert_eq!(
        calls(&call_log),
        [
            "physical:update_value:15:Object(String(\"index\"))",
            "physical:update_value_by_label:object_value:Object(Scalar(Null))",
            "physical:update_value:16:ObjectWithScaleOrLength { value: Integer(7), scale_or_length: -3 }",
            "physical:update_value_by_label:scaled_object:ObjectWithScaleOrLength { value: Bytes([9]), scale_or_length: 99 }",
        ]
    );
}

#[test]
fn stat_filter_commits_close_statistics_before_downstream_close_error() {
    let collector = Arc::new(StatsCollector::new("probe", Duration::from_secs(1)));
    let stat_filter = Arc::new(StatFilter::new(Arc::clone(&collector)));
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(call_log, true);
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    assert!(std::ptr::eq(
        stat_filter.result_set_stat(),
        collector.result_set_stat()
    ));
    chain.add_result_set(stat_filter);

    chain.result_set_open_after(&context).unwrap();
    context.record_fetch_row_count(3);
    assert_eq!(collector.result_set_stat().opening_count(), 1);
    assert_eq!(
        chain.result_set_close(&physical, &context),
        Err(DruidError::DriverError("close failed".to_string()))
    );

    let stat = collector.result_set_stat();
    assert_eq!(stat.open_count(), 1);
    assert_eq!(stat.opening_count(), 0);
    assert_eq!(stat.fetch_row_count(), 3);
    assert_eq!(stat.close_count(), 1);
    assert_eq!(context.close_count(), 0);
}

#[tokio::test]
async fn pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ScalarShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        73,
        "sqlite-scalar-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    // Filter 在 raw ResultSet 前短路，因此未调用 next 也能返回改写值。
    assert_eq!(
        result_set.string(&mut connection, 1).unwrap(),
        Some("filtered-index".to_string())
    );
    assert_eq!(
        result_set
            .string_by_label(&mut connection, "string")
            .unwrap(),
        Some("filtered-label".to_string())
    );
    assert!(result_set.boolean(&mut connection, 2).unwrap());
    assert!(!result_set
        .boolean_by_label(&mut connection, "boolean")
        .unwrap());
    assert_eq!(result_set.byte(&mut connection, 3).unwrap(), 11);
    assert_eq!(
        result_set.byte_by_label(&mut connection, "byte").unwrap(),
        12
    );
    assert_eq!(result_set.short(&mut connection, 4).unwrap(), 21);
    assert_eq!(
        result_set.short_by_label(&mut connection, "short").unwrap(),
        22
    );
    assert_eq!(result_set.int(&mut connection, 5).unwrap(), 31);
    assert_eq!(result_set.int_by_label(&mut connection, "int").unwrap(), 32);
    assert_eq!(result_set.long(&mut connection, 6).unwrap(), 41);
    assert_eq!(
        result_set.long_by_label(&mut connection, "long").unwrap(),
        42
    );
    assert_eq!(result_set.float(&mut connection, 7).unwrap(), 51.5);
    assert_eq!(
        result_set.float_by_label(&mut connection, "float").unwrap(),
        52.5
    );
    assert_eq!(result_set.double(&mut connection, 8).unwrap(), 61.5);
    assert_eq!(
        result_set
            .double_by_label(&mut connection, "double")
            .unwrap(),
        62.5
    );
    assert_eq!(
        result_set.bytes(&mut connection, 9).unwrap(),
        Some(vec![7, 8])
    );
    assert_eq!(
        result_set.bytes_by_label(&mut connection, "bytes").unwrap(),
        Some(vec![9, 10])
    );

    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_string:1",
            "result_set_get_string_by_label:string",
            "result_set_get_boolean:2",
            "result_set_get_boolean_by_label:boolean",
            "result_set_get_byte:3",
            "result_set_get_byte_by_label:byte",
            "result_set_get_short:4",
            "result_set_get_short_by_label:short",
            "result_set_get_int:5",
            "result_set_get_int_by_label:int",
            "result_set_get_long:6",
            "result_set_get_long_by_label:long",
            "result_set_get_float:7",
            "result_set_get_float_by_label:float",
            "result_set_get_double:8",
            "result_set_get_double_by_label:double",
            "result_set_get_bytes:9",
            "result_set_get_bytes_by_label:bytes",
        ]
    );

    assert_eq!(
        result_set.double_by_label(&mut connection, "fail"),
        Err(DruidError::DriverError(
            "filtered double failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn pooled_result_set_preserves_decimal_temporal_and_calendar_filter_overloads() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StrongGetterShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        74,
        "sqlite-strong-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    assert_eq!(
        result_set.big_decimal(&mut connection, 1).unwrap(),
        Some(BigDecimal::from(101))
    );
    assert_eq!(
        result_set
            .big_decimal_by_label(&mut connection, "decimal")
            .unwrap(),
        Some(BigDecimal::from(102))
    );
    assert_eq!(
        result_set
            .big_decimal_with_scale(&mut connection, 2, 7)
            .unwrap(),
        Some(BigDecimal::from(103))
    );
    assert_eq!(
        result_set
            .big_decimal_by_label_with_scale(&mut connection, "decimal_scale", 8)
            .unwrap(),
        Some(BigDecimal::from(104))
    );

    assert_eq!(
        result_set.date(&mut connection, 3).unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 1).unwrap())
    );
    assert_eq!(
        result_set.date_by_label(&mut connection, "date").unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 2).unwrap())
    );
    assert_eq!(
        result_set
            .date_with_calendar(&mut connection, 4, None)
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 3).unwrap())
    );
    assert_eq!(
        result_set
            .date_by_label_with_calendar(
                &mut connection,
                "date_calendar",
                Some(JdbcCalendar::new("UTC").unwrap()),
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 3).unwrap())
    );

    assert_eq!(
        result_set.time(&mut connection, 5).unwrap(),
        Some(NaiveTime::from_hms_opt(1, 2, 3).unwrap())
    );
    assert_eq!(
        result_set.time_by_label(&mut connection, "time").unwrap(),
        Some(NaiveTime::from_hms_opt(2, 3, 4).unwrap())
    );
    assert_eq!(
        result_set
            .time_with_calendar(&mut connection, 6, None)
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        result_set
            .time_by_label_with_calendar(
                &mut connection,
                "time_calendar",
                Some(JdbcCalendar::new("UTC").unwrap()),
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );

    let index_timestamp = NaiveDate::from_ymd_opt(2031, 1, 1)
        .unwrap()
        .and_hms_opt(1, 2, 3)
        .unwrap();
    let label_timestamp = NaiveDate::from_ymd_opt(2031, 1, 2)
        .unwrap()
        .and_hms_opt(2, 3, 4)
        .unwrap();
    let calendar_timestamp = NaiveDate::from_ymd_opt(2031, 1, 3)
        .unwrap()
        .and_hms_opt(3, 4, 5)
        .unwrap();
    assert_eq!(
        result_set.timestamp(&mut connection, 7).unwrap(),
        Some(index_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label(&mut connection, "timestamp")
            .unwrap(),
        Some(label_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_with_calendar(&mut connection, 8, None)
            .unwrap(),
        Some(calendar_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label_with_calendar(
                &mut connection,
                "timestamp_calendar",
                Some(JdbcCalendar::new("UTC").unwrap()),
            )
            .unwrap(),
        Some(calendar_timestamp)
    );

    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_big_decimal:1",
            "result_set_get_big_decimal_by_label:decimal",
            "result_set_get_big_decimal_with_scale:2:7",
            "result_set_get_big_decimal_by_label_with_scale:decimal_scale:8",
            "result_set_get_date:3",
            "result_set_get_date_by_label:date",
            "result_set_get_date_with_calendar:4:specified:null",
            "result_set_get_date_by_label_with_calendar:date_calendar:specified:UTC",
            "result_set_get_time:5",
            "result_set_get_time_by_label:time",
            "result_set_get_time_with_calendar:6:specified:null",
            "result_set_get_time_by_label_with_calendar:time_calendar:specified:UTC",
            "result_set_get_timestamp:7",
            "result_set_get_timestamp_by_label:timestamp",
            "result_set_get_timestamp_with_calendar:8:specified:null",
            "result_set_get_timestamp_by_label_with_calendar:timestamp_calendar:specified:UTC",
        ]
    );

    assert_eq!(
        result_set.timestamp_by_label_with_calendar(
            &mut connection,
            "fail",
            Some(JdbcCalendar::new("UTC").unwrap()),
        ),
        Err(DruidError::DriverError(
            "filtered temporal failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ObjectShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        75,
        "sqlite-object-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    let empty_map = JdbcTypeMap::new();
    let mut populated_map = JdbcTypeMap::new();
    populated_map.insert(
        "example.address",
        JdbcTargetType::Custom("example.Address".to_string()),
    );
    let custom_target = JdbcTargetType::Custom("example.Target".to_string());

    assert_eq!(
        result_set.object(&mut connection, 1).unwrap(),
        Value::Int(901)
    );
    assert_eq!(
        result_set
            .object_by_label(&mut connection, "value")
            .unwrap(),
        Value::String("filtered-label-object".to_string())
    );
    assert_eq!(
        result_set
            .object_with_type_map(&mut connection, 2, None)
            .unwrap(),
        JdbcObject::String("filtered-map:null".to_string())
    );
    assert_eq!(
        result_set
            .object_with_type_map(&mut connection, 3, Some(&empty_map))
            .unwrap(),
        JdbcObject::String("filtered-map:empty".to_string())
    );
    assert_eq!(
        result_set
            .object_by_label_with_type_map(&mut connection, "mapped", Some(&populated_map),)
            .unwrap(),
        JdbcObject::String(
            "filtered-label-map:example.address=Custom(\"example.Address\")".to_string()
        )
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 4, &custom_target)
            .unwrap(),
        JdbcObject::String("filtered-typed:Custom(\"example.Target\")".to_string())
    );
    assert_eq!(
        result_set
            .object_typed_by_label(&mut connection, "typed", &JdbcTargetType::String)
            .unwrap(),
        JdbcObject::String("filtered-label-typed:String".to_string())
    );

    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_object:1",
            "result_set_get_object_by_label:value",
            "result_set_get_object_with_type_map:2:null",
            "result_set_get_object_with_type_map:3:empty",
            "result_set_get_object_by_label_with_type_map:mapped:example.address=Custom(\"example.Address\")",
            "result_set_get_object_typed:4:Custom(\"example.Target\")",
            "result_set_get_object_typed_by_label:typed:String",
        ]
    );

    assert_eq!(
        result_set.object_typed_by_label(
            &mut connection,
            "fail",
            &JdbcTargetType::Custom("example.Fail".to_string()),
        ),
        Err(DruidError::DriverError(
            "filtered object failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn pooled_result_set_routes_resource_getters_and_real_sqlite_streams() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let shared_stream = JdbcInputStream::from_bytes([1, 2, 3]);
    let shared_reader = JdbcReader::from_string("迁移");
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ResourceShortCircuitFilter {
        calls: Arc::clone(&call_log),
        stream: shared_stream,
        reader: shared_reader,
    }));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        76,
        "sqlite-resource-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    macro_rules! assert_none_resource_pair {
        ($index:ident, $label:ident, $position:expr, $name:literal) => {
            assert!(result_set
                .$index(&mut connection, $position)
                .unwrap()
                .is_none());
            assert!(result_set.$label(&mut connection, $name).unwrap().is_none());
        };
    }

    assert_none_resource_pair!(reference, reference_by_label, 1, "ref");
    assert_none_resource_pair!(blob, blob_by_label, 2, "blob");
    assert_none_resource_pair!(clob, clob_by_label, 3, "clob");
    assert_none_resource_pair!(array, array_by_label, 4, "array");
    assert_none_resource_pair!(url, url_by_label, 5, "url");
    assert_none_resource_pair!(row_id, row_id_by_label, 6, "row_id");
    assert_none_resource_pair!(n_clob, n_clob_by_label, 7, "n_clob");
    assert_none_resource_pair!(sql_xml, sql_xml_by_label, 8, "sql_xml");

    let stream_by_index = result_set
        .ascii_stream(&mut connection, 9)
        .unwrap()
        .unwrap();
    let stream_by_label = result_set
        .ascii_stream_by_label(&mut connection, "ascii")
        .unwrap()
        .unwrap();
    let mut first_byte = [0_u8; 1];
    assert_eq!(stream_by_index.read(&mut first_byte).unwrap(), 1);
    assert_eq!(first_byte, [1]);
    assert_eq!(stream_by_label.read_to_end().unwrap(), [2, 3]);

    assert_none_resource_pair!(unicode_stream, unicode_stream_by_label, 10, "unicode");
    assert_none_resource_pair!(binary_stream, binary_stream_by_label, 11, "binary");

    let reader_by_index = result_set
        .character_stream(&mut connection, 12)
        .unwrap()
        .unwrap();
    let reader_by_label = result_set
        .character_stream_by_label(&mut connection, "character")
        .unwrap()
        .unwrap();
    let mut first_unit = [0_u16; 1];
    assert_eq!(reader_by_index.read_utf16(&mut first_unit).unwrap(), 1);
    assert_eq!(first_unit[0], '迁' as u16);
    assert_eq!(reader_by_label.read_to_string().unwrap(), "移");

    assert_none_resource_pair!(
        n_character_stream,
        n_character_stream_by_label,
        13,
        "n_character"
    );

    assert_eq!(
        result_set.sql_xml_by_label(&mut connection, "fail"),
        Err(DruidError::DriverError(
            "filtered SQLXML failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(calls(&call_log).len(), 27);
    result_set.close_with_connection(&mut connection).unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite 行集经过默认 Filter，
    // 验证流资源的实际读取。Toasty 当前行集未暴露 SQL alias，故主机证据使用
    // JDBC 1-based 索引；标签重载身份由上方精确物理探针独立证明。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        77,
        "sqlite-resource-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(
            &mut connection,
            "SELECT X'010203' AS payload, '你好' AS text_value",
        )
        .await
        .unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set
            .binary_stream(&mut connection, 1)
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        [1, 2, 3]
    );
    assert_eq!(
        result_set
            .n_character_stream(&mut connection, 2)
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "你好"
    );
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn pooled_result_set_routes_navigation_properties_and_real_sqlite_state() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(NavigationShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        78,
        "sqlite-navigation-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1")
        .await
        .unwrap();

    assert!(!result_set.was_null(&mut connection).unwrap());
    assert!(result_set.previous(&mut connection).unwrap());
    assert!(!result_set.is_before_first(&mut connection).unwrap());
    assert!(result_set.is_after_last(&mut connection).unwrap());
    assert!(result_set.is_first(&mut connection).unwrap());
    assert!(!result_set.is_last(&mut connection).unwrap());
    result_set.before_first(&mut connection).unwrap();
    result_set.after_last(&mut connection).unwrap();
    assert!(!result_set.first(&mut connection).unwrap());
    assert!(!result_set.last(&mut connection).unwrap());
    assert_eq!(result_set.row(&mut connection).unwrap(), 99);
    assert!(result_set.absolute(&mut connection, 50).unwrap());
    assert!(result_set.relative(&mut connection, -20).unwrap());
    result_set
        .set_fetch_direction(&mut connection, 2000)
        .unwrap();
    assert_eq!(result_set.fetch_direction(&mut connection).unwrap(), 2000);
    result_set.set_fetch_size(&mut connection, 128).unwrap();
    assert_eq!(result_set.fetch_size(&mut connection).unwrap(), 128);
    assert_eq!(result_set.result_set_type(&mut connection).unwrap(), 2004);
    assert_eq!(result_set.concurrency(&mut connection).unwrap(), 2007);
    assert_eq!(result_set.holdability(&mut connection).unwrap(), 2);
    assert_eq!(
        result_set.cursor_name(&mut connection).unwrap(),
        Some("filtered-cursor".to_string())
    );
    assert!(!result_set.row_updated(&mut connection).unwrap());
    assert!(result_set.row_inserted(&mut connection).unwrap());
    assert!(!result_set.row_deleted(&mut connection).unwrap());
    assert_eq!(
        result_set.find_column(&mut connection, "named").unwrap(),
        88
    );
    assert!(!result_set
        .is_closed_with_connection(&mut connection)
        .unwrap());
    assert_eq!(calls(&call_log).len(), 26);
    assert_eq!(
        result_set.find_column(&mut connection, "fail"),
        Err(DruidError::DriverError(
            "filtered findColumn failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite RowSet 经默认 Filter 执行游标状态机。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        79,
        "sqlite-navigation-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 10 AS value UNION ALL SELECT 20")
        .await
        .unwrap();
    assert!(result_set.is_before_first(&mut connection).unwrap());
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(result_set.row(&mut connection).unwrap(), 1);
    assert!(result_set.last(&mut connection).unwrap());
    assert_eq!(result_set.row(&mut connection).unwrap(), 2);
    assert!(result_set.previous(&mut connection).unwrap());
    assert_eq!(result_set.int(&mut connection, 1).unwrap(), 10);
    result_set.after_last(&mut connection).unwrap();
    assert!(result_set.is_after_last(&mut connection).unwrap());
    result_set.before_first(&mut connection).unwrap();
    assert!(result_set.is_before_first(&mut connection).unwrap());
    assert!(!result_set
        .is_closed_with_connection(&mut connection)
        .unwrap());
    result_set.close_with_connection(&mut connection).unwrap();
    assert!(result_set.is_closed());
}

#[tokio::test]
async fn pooled_result_set_routes_row_mutations_and_preserves_sqlite_capability_errors() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let fail_refresh = Arc::new(AtomicBool::new(false));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(RowMutationShortCircuitFilter {
        calls: Arc::clone(&call_log),
        fail_refresh: Arc::clone(&fail_refresh),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        85,
        "sqlite-row-mutation-short-circuit".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    // SOURCE_PARITY / V2_MIRRORED：Filter 可以短路七个 raw mutation 调用。
    result_set.insert_row(&mut connection).unwrap();
    result_set.update_row(&mut connection).unwrap();
    result_set.delete_row(&mut connection).unwrap();
    result_set.refresh_row(&mut connection).unwrap();
    result_set.cancel_row_updates(&mut connection).unwrap();
    result_set.move_to_insert_row(&mut connection).unwrap();
    result_set.move_to_current_row(&mut connection).unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "insert_row",
            "update_row",
            "delete_row",
            "refresh_row",
            "cancel_row_updates",
            "move_to_insert_row",
            "move_to_current_row",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误必须在原 Statement 计数，
    // 且短路后不能触达物理 ResultSet。
    fail_refresh.store(true, Ordering::Release);
    assert_eq!(
        result_set.refresh_row(&mut connection),
        Err(DruidError::DriverError(
            "filtered refreshRow failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(calls(&call_log).last().unwrap(), "refresh_row");
    result_set.close_with_connection(&mut connection).unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite 的只读 RowSet 必须经默认
    // Filter 保留每个物理方法自己的精确 capability error，而不是统一报错。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        86,
        "sqlite-row-mutation-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    macro_rules! assert_capability_error {
        ($method:ident, $operation:literal) => {
            assert_eq!(
                result_set.$method(&mut connection),
                Err(DruidError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    assert_capability_error!(insert_row, "result_set_insert_row");
    assert_capability_error!(update_row, "result_set_update_row");
    assert_capability_error!(delete_row, "result_set_delete_row");
    assert_capability_error!(refresh_row, "result_set_refresh_row");
    assert_capability_error!(cancel_row_updates, "result_set_cancel_row_updates");
    assert_capability_error!(move_to_insert_row, "result_set_move_to_insert_row");
    assert_capability_error!(move_to_current_row, "result_set_move_to_current_row");
    assert_eq!(statement.exception_count(), 7);
    assert!(!result_set.is_closed());
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn pooled_result_set_routes_scalar_updates_and_preserves_sqlite_capability_errors() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ScalarUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        87,
        "sqlite-scalar-update-short-circuit".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_opt(10, 11, 12).unwrap();

    macro_rules! invoke_pair {
        ($index:ident, $label:ident, $index_value:expr, $label_value:expr) => {
            result_set.$index(&mut connection, 1, $index_value).unwrap();
            result_set
                .$label(&mut connection, "value", $label_value)
                .unwrap();
        };
    }

    // SOURCE_PARITY / V2_MIRRORED：真实池化结果集上的 28 个调用全部被
    // 各自的 Filter 方法短路，证明没有退化成单一通用 update 回调。
    result_set.update_null(&mut connection, 1).unwrap();
    result_set
        .update_null_by_label(&mut connection, "value")
        .unwrap();
    invoke_pair!(update_boolean, update_boolean_by_label, true, false);
    invoke_pair!(update_byte, update_byte_by_label, -8, 8);
    invoke_pair!(update_short, update_short_by_label, -16, 16);
    invoke_pair!(update_int, update_int_by_label, -32, 32);
    invoke_pair!(update_long, update_long_by_label, -64, 64);
    invoke_pair!(update_float, update_float_by_label, 1.25, -1.25);
    invoke_pair!(update_double, update_double_by_label, 2.5, -2.5);
    invoke_pair!(
        update_big_decimal,
        update_big_decimal_by_label,
        Some(BigDecimal::from_str("12.340").unwrap()),
        None
    );
    invoke_pair!(
        update_string,
        update_string_by_label,
        Some("index".to_string()),
        None
    );
    invoke_pair!(
        update_bytes,
        update_bytes_by_label,
        Some(vec![0, 255]),
        None
    );
    invoke_pair!(update_date, update_date_by_label, Some(date), None);
    invoke_pair!(update_time, update_time_by_label, Some(time), None);
    invoke_pair!(
        update_timestamp,
        update_timestamp_by_label,
        Some(date.and_time(time)),
        None
    );
    assert_eq!(calls(&call_log).len(), 28);

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误归属原 Statement，
    // 且失败的标签重载不触达只读 SQLite RowSet。
    assert_eq!(
        result_set.update_timestamp_by_label(&mut connection, "fail", None),
        Err(DruidError::DriverError(
            "filtered scalar update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();

    // VALUE_ADD / V5_HOST：默认 Filter 对真实 Toasty SQLite 的 28 个
    // 调用保持物理只读能力错误，并区分下标与标签终端。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        88,
        "sqlite-scalar-update-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    macro_rules! assert_index_error {
        ($method:ident $(, $value:expr)?) => {
            assert_eq!(
                result_set.$method(&mut connection, 1 $(, $value)?),
                Err(DruidError::UnsupportedOperation {
                    operation: "result_set_update_value",
                })
            );
        };
    }
    macro_rules! assert_label_error {
        ($method:ident $(, $value:expr)?) => {
            assert_eq!(
                result_set.$method(&mut connection, "value" $(, $value)?),
                Err(DruidError::UnsupportedOperation {
                    operation: "result_set_update_value_by_label",
                })
            );
        };
    }
    macro_rules! assert_pair_error {
        ($index:ident, $label:ident, $index_value:expr, $label_value:expr) => {
            assert_index_error!($index, $index_value);
            assert_label_error!($label, $label_value);
        };
    }

    assert_index_error!(update_null);
    assert_label_error!(update_null_by_label);
    assert_pair_error!(update_boolean, update_boolean_by_label, true, false);
    assert_pair_error!(update_byte, update_byte_by_label, -8, 8);
    assert_pair_error!(update_short, update_short_by_label, -16, 16);
    assert_pair_error!(update_int, update_int_by_label, -32, 32);
    assert_pair_error!(update_long, update_long_by_label, -64, 64);
    assert_pair_error!(update_float, update_float_by_label, 1.25, -1.25);
    assert_pair_error!(update_double, update_double_by_label, 2.5, -2.5);
    assert_pair_error!(
        update_big_decimal,
        update_big_decimal_by_label,
        Some(BigDecimal::from(1)),
        None
    );
    assert_pair_error!(
        update_string,
        update_string_by_label,
        Some("index".to_string()),
        None
    );
    assert_pair_error!(
        update_bytes,
        update_bytes_by_label,
        Some(vec![0, 255]),
        None
    );
    assert_pair_error!(update_date, update_date_by_label, Some(date), None);
    assert_pair_error!(update_time, update_time_by_label, Some(time), None);
    assert_pair_error!(
        update_timestamp,
        update_timestamp_by_label,
        Some(date.and_time(time)),
        None
    );
    assert_eq!(statement.exception_count(), 28);
    assert!(!result_set.is_closed());
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn pooled_result_set_routes_object_updates_and_preserves_sqlite_capability_errors() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    let vendor_object = JdbcOpaqueObject::new(Arc::new(FilterVendorObjectProbe { id: 99 }));
    filter_chain.add_result_set(Arc::new(ObjectUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
        expected_custom: vendor_object.clone(),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        89,
        "sqlite-object-update-short-circuit".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    // SOURCE_PARITY / V2_MIRRORED：plain 与 scaleOrLength 的 index/label
    // 四重载均可独立短路，SQL NULL 与负 scaleOrLength 不被改写。
    result_set
        .update_object(&mut connection, 15, JdbcObject::Custom(vendor_object))
        .unwrap();
    result_set
        .update_object_by_label(&mut connection, "object_value", Value::Null.into())
        .unwrap();
    result_set
        .update_object_with_scale_or_length(&mut connection, 16, JdbcObject::Integer(7), -3)
        .unwrap();
    result_set
        .update_object_by_label_with_scale_or_length(
            &mut connection,
            "scaled_object",
            JdbcObject::Bytes(vec![9]),
            99,
        )
        .unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_object:15:custom:example.FilterVendorObject:99",
            "result_set_update_object_by_label:object_value:Scalar(Null)",
            "result_set_update_object_with_scale_or_length:16:Integer(7):-3",
            "result_set_update_object_by_label_with_scale_or_length:scaled_object:Bytes([9]):99",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误在原 Statement 计数一次，
    // 且短路后不触达真实只读 RowSet。
    assert_eq!(
        result_set.update_object_by_label(&mut connection, "fail", Value::Null.into()),
        Err(DruidError::DriverError(
            "filtered updateObject failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();

    // VALUE_ADD / V5_HOST：默认 Filter 必须把四重载送至真实 Toasty SQLite
    // RowSet，并保持 index/label 两种精确能力错误。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        90,
        "sqlite-object-update-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    assert_eq!(
        result_set.update_object(&mut connection, 15, JdbcObject::String("index".to_string())),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value",
        })
    );
    assert_eq!(
        result_set.update_object_by_label(&mut connection, "object_value", Value::Null.into()),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value_by_label",
        })
    );
    assert_eq!(
        result_set.update_object_with_scale_or_length(
            &mut connection,
            16,
            JdbcObject::Integer(7),
            -3,
        ),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value",
        })
    );
    assert_eq!(
        result_set.update_object_by_label_with_scale_or_length(
            &mut connection,
            "scaled_object",
            JdbcObject::Bytes(vec![9]),
            99,
        ),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value_by_label",
        })
    );
    assert_eq!(statement.exception_count(), 4);
    assert!(!result_set.is_closed());
    result_set.close_with_connection(&mut connection).unwrap();
}

#[test]
fn result_set_resource_update_filter_defaults_preserve_all_fourteen_overloads() {
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&physical_log), false);
    let context = ResultSetFilterContext::new();
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let row_id = JdbcRowId::new([1, 2, 3]);

    // SOURCE_PARITY / V2_MIRRORED：七个资源类型的 index/label 重载分别进入
    // Filter，Java null 与 RowId 值身份在默认末端保持不变。
    filter_chain
        .result_set_update_reference(&physical, &context, 1, None)
        .unwrap();
    filter_chain
        .result_set_update_reference_by_label(&physical, &context, "ref", None)
        .unwrap();
    filter_chain
        .result_set_update_blob(&physical, &context, 2, None)
        .unwrap();
    filter_chain
        .result_set_update_blob_by_label(&physical, &context, "blob", None)
        .unwrap();
    filter_chain
        .result_set_update_clob(&physical, &context, 3, None)
        .unwrap();
    filter_chain
        .result_set_update_clob_by_label(&physical, &context, "clob", None)
        .unwrap();
    filter_chain
        .result_set_update_array(&physical, &context, 4, None)
        .unwrap();
    filter_chain
        .result_set_update_array_by_label(&physical, &context, "array", None)
        .unwrap();
    filter_chain
        .result_set_update_row_id(&physical, &context, 5, Some(row_id.clone()))
        .unwrap();
    filter_chain
        .result_set_update_row_id_by_label(&physical, &context, "row_id", Some(row_id))
        .unwrap();
    filter_chain
        .result_set_update_n_clob(&physical, &context, 6, None)
        .unwrap();
    filter_chain
        .result_set_update_n_clob_by_label(&physical, &context, "n_clob", None)
        .unwrap();
    filter_chain
        .result_set_update_sql_xml(&physical, &context, 7, None)
        .unwrap();
    filter_chain
        .result_set_update_sql_xml_by_label(&physical, &context, "sql_xml", None)
        .unwrap();

    assert_eq!(
        calls(&physical_log),
        [
            "physical:update_reference:1:None",
            "physical:update_reference_by_label:ref:None",
            "physical:update_blob:2:None",
            "physical:update_blob_by_label:blob:None",
            "physical:update_clob:3:None",
            "physical:update_clob_by_label:clob:None",
            "physical:update_array:4:None",
            "physical:update_array_by_label:array:None",
            "physical:update_row_id:5:Some(JdbcRowId { bytes: [1, 2, 3] })",
            "physical:update_row_id_by_label:row_id:Some(JdbcRowId { bytes: [1, 2, 3] })",
            "physical:update_n_clob:6:None",
            "physical:update_n_clob_by_label:n_clob:None",
            "physical:update_sql_xml:7:None",
            "physical:update_sql_xml_by_label:sql_xml:None",
        ]
    );
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn pooled_result_set_routes_resource_updates_and_preserves_sqlite_capability_errors() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ResourceUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        91,
        "sqlite-resource-update-short-circuit".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();
    let row_id = JdbcRowId::new([4, 5, 6]);

    result_set
        .update_reference(&mut connection, 1, None)
        .unwrap();
    result_set
        .update_reference_by_label(&mut connection, "ref", None)
        .unwrap();
    result_set.update_blob(&mut connection, 2, None).unwrap();
    result_set
        .update_blob_by_label(&mut connection, "blob", None)
        .unwrap();
    result_set.update_clob(&mut connection, 3, None).unwrap();
    result_set
        .update_clob_by_label(&mut connection, "clob", None)
        .unwrap();
    result_set.update_array(&mut connection, 4, None).unwrap();
    result_set
        .update_array_by_label(&mut connection, "array", None)
        .unwrap();
    result_set
        .update_row_id(&mut connection, 5, Some(&row_id))
        .unwrap();
    result_set
        .update_row_id_by_label(&mut connection, "row_id", Some(&row_id))
        .unwrap();
    result_set.update_n_clob(&mut connection, 6, None).unwrap();
    result_set
        .update_n_clob_by_label(&mut connection, "n_clob", None)
        .unwrap();
    result_set.update_sql_xml(&mut connection, 7, None).unwrap();
    result_set
        .update_sql_xml_by_label(&mut connection, "sql_xml", None)
        .unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_reference:1:None",
            "result_set_update_reference_by_label:ref:None",
            "result_set_update_blob:2:None",
            "result_set_update_blob_by_label:blob:None",
            "result_set_update_clob:3:None",
            "result_set_update_clob_by_label:clob:None",
            "result_set_update_array:4:None",
            "result_set_update_array_by_label:array:None",
            "result_set_update_row_id:5:Some(JdbcRowId { bytes: [4, 5, 6] })",
            "result_set_update_row_id_by_label:row_id:Some(JdbcRowId { bytes: [4, 5, 6] })",
            "result_set_update_n_clob:6:None",
            "result_set_update_n_clob_by_label:n_clob:None",
            "result_set_update_sql_xml:7:None",
            "result_set_update_sql_xml_by_label:sql_xml:None",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误只在原 Statement 分类一次，
    // 且不会触达只读 RowSet。
    assert_eq!(
        result_set.update_sql_xml_by_label(&mut connection, "fail", None),
        Err(DruidError::DriverError(
            "filtered resource update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();

    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        92,
        "sqlite-resource-update-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    macro_rules! assert_resource_unsupported {
        ($expression:expr, $operation:literal) => {
            assert_eq!(
                $expression,
                Err(DruidError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    // VALUE_ADD / V5_HOST：默认 Filter 保留真实 Toasty SQLite 的 14 个精确
    // capability error，而不是统一成通用 update 错误。
    assert_resource_unsupported!(
        result_set.update_reference(&mut connection, 1, None),
        "result_set_update_ref"
    );
    assert_resource_unsupported!(
        result_set.update_reference_by_label(&mut connection, "ref", None),
        "result_set_update_ref_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_blob(&mut connection, 2, None),
        "result_set_update_blob"
    );
    assert_resource_unsupported!(
        result_set.update_blob_by_label(&mut connection, "blob", None),
        "result_set_update_blob_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_clob(&mut connection, 3, None),
        "result_set_update_clob"
    );
    assert_resource_unsupported!(
        result_set.update_clob_by_label(&mut connection, "clob", None),
        "result_set_update_clob_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_array(&mut connection, 4, None),
        "result_set_update_array"
    );
    assert_resource_unsupported!(
        result_set.update_array_by_label(&mut connection, "array", None),
        "result_set_update_array_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_row_id(&mut connection, 5, Some(&row_id)),
        "result_set_update_row_id"
    );
    assert_resource_unsupported!(
        result_set.update_row_id_by_label(&mut connection, "row_id", Some(&row_id)),
        "result_set_update_row_id_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_n_clob(&mut connection, 6, None),
        "result_set_update_n_clob"
    );
    assert_resource_unsupported!(
        result_set.update_n_clob_by_label(&mut connection, "n_clob", None),
        "result_set_update_n_clob_by_label"
    );
    assert_resource_unsupported!(
        result_set.update_sql_xml(&mut connection, 7, None),
        "result_set_update_sql_xml"
    );
    assert_resource_unsupported!(
        result_set.update_sql_xml_by_label(&mut connection, "sql_xml", None),
        "result_set_update_sql_xml_by_label"
    );
    assert_eq!(statement.exception_count(), 14);
    assert!(!result_set.is_closed());
    result_set.close_with_connection(&mut connection).unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn result_set_lob_stream_update_filter_defaults_preserve_all_twelve_overloads() {
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&physical_log), false);
    let context = ResultSetFilterContext::new();
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let blob = JdbcInputStream::from_bytes([1, 2, 3]);
    let clob = JdbcReader::from_string("甲乙");
    let n_clob = JdbcReader::from_string("丙丁");

    // SOURCE_PARITY / V2_MIRRORED：Java Filter/FilterChainImpl 的 Blob、
    // Clob、NClob × index/label × unspecified/long 12 个重载逐一穿透。
    filter_chain
        .result_set_update_blob_stream(&physical, &context, 1, Some(blob.clone()))
        .unwrap();
    filter_chain
        .result_set_update_blob_stream_by_label(&physical, &context, "blob", None)
        .unwrap();
    filter_chain
        .result_set_update_blob_stream_with_length(&physical, &context, 2, Some(blob.clone()), -7)
        .unwrap();
    filter_chain
        .result_set_update_blob_stream_by_label_with_length(
            &physical,
            &context,
            "blob_long",
            None,
            i64::MAX,
        )
        .unwrap();
    filter_chain
        .result_set_update_clob_reader(&physical, &context, 3, Some(clob.clone()))
        .unwrap();
    filter_chain
        .result_set_update_clob_reader_by_label(&physical, &context, "clob", None)
        .unwrap();
    filter_chain
        .result_set_update_clob_reader_with_length(&physical, &context, 4, Some(clob.clone()), -11)
        .unwrap();
    filter_chain
        .result_set_update_clob_reader_by_label_with_length(
            &physical,
            &context,
            "clob_long",
            None,
            13,
        )
        .unwrap();
    filter_chain
        .result_set_update_n_clob_reader(&physical, &context, 5, Some(n_clob.clone()))
        .unwrap();
    filter_chain
        .result_set_update_n_clob_reader_by_label(&physical, &context, "n_clob", None)
        .unwrap();
    filter_chain
        .result_set_update_n_clob_reader_with_length(
            &physical,
            &context,
            6,
            Some(n_clob.clone()),
            -17,
        )
        .unwrap();
    filter_chain
        .result_set_update_n_clob_reader_by_label_with_length(
            &physical,
            &context,
            "n_clob_long",
            None,
            19,
        )
        .unwrap();

    assert_eq!(
        calls(&physical_log),
        [
            "physical:update_blob_stream:1:true:Unspecified",
            "physical:update_blob_stream_by_label:blob:false:Unspecified",
            "physical:update_blob_stream:2:true:Long(-7)",
            "physical:update_blob_stream_by_label:blob_long:false:Long(9223372036854775807)",
            "physical:update_clob_reader:3:true:Unspecified",
            "physical:update_clob_reader_by_label:clob:false:Unspecified",
            "physical:update_clob_reader:4:true:Long(-11)",
            "physical:update_clob_reader_by_label:clob_long:false:Long(13)",
            "physical:update_n_clob_reader:5:true:Unspecified",
            "physical:update_n_clob_reader_by_label:n_clob:false:Unspecified",
            "physical:update_n_clob_reader:6:true:Long(-17)",
            "physical:update_n_clob_reader_by_label:n_clob_long:false:Long(19)",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：默认 Filter 只克隆共享句柄，不预读、
    // 不关闭，也不校正负长度或超大 long。
    assert_eq!(blob.read_to_end().unwrap(), [1, 2, 3]);
    assert_eq!(clob.read_to_string().unwrap(), "甲乙");
    assert_eq!(n_clob.read_to_string().unwrap(), "丙丁");
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn pooled_result_set_routes_lob_stream_updates_and_preserves_sqlite_capability_errors() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(LobStreamUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        93,
        "sqlite-lob-stream-update-short-circuit".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT X'010203' AS value")
        .await
        .unwrap();
    let blob = JdbcInputStream::from_bytes([1, 2, 3]);
    let clob = JdbcReader::from_string("甲乙");
    let n_clob = JdbcReader::from_string("丙丁");

    result_set
        .update_blob_stream(&mut connection, 1, Some(&blob))
        .unwrap();
    result_set
        .update_blob_stream_by_label(&mut connection, "blob", None)
        .unwrap();
    result_set
        .update_blob_stream_with_length(&mut connection, 2, Some(&blob), -7)
        .unwrap();
    result_set
        .update_blob_stream_by_label_with_length(&mut connection, "blob_long", None, i64::MAX)
        .unwrap();
    result_set
        .update_clob_reader(&mut connection, 3, Some(&clob))
        .unwrap();
    result_set
        .update_clob_reader_by_label(&mut connection, "clob", None)
        .unwrap();
    result_set
        .update_clob_reader_with_length(&mut connection, 4, Some(&clob), -11)
        .unwrap();
    result_set
        .update_clob_reader_by_label_with_length(&mut connection, "clob_long", None, 13)
        .unwrap();
    result_set
        .update_n_clob_reader(&mut connection, 5, Some(&n_clob))
        .unwrap();
    result_set
        .update_n_clob_reader_by_label(&mut connection, "n_clob", None)
        .unwrap();
    result_set
        .update_n_clob_reader_with_length(&mut connection, 6, Some(&n_clob), -17)
        .unwrap();
    result_set
        .update_n_clob_reader_by_label_with_length(&mut connection, "n_clob_long", None, 19)
        .unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_blob_stream:1:some",
            "result_set_update_blob_stream_by_label:blob:none",
            "result_set_update_blob_stream_with_length:2:some:-7",
            "result_set_update_blob_stream_by_label_with_length:blob_long:none:9223372036854775807",
            "result_set_update_clob_reader:3:some",
            "result_set_update_clob_reader_by_label:clob:none",
            "result_set_update_clob_reader_with_length:4:some:-11",
            "result_set_update_clob_reader_by_label_with_length:clob_long:none:13",
            "result_set_update_n_clob_reader:5:some",
            "result_set_update_n_clob_reader_by_label:n_clob:none",
            "result_set_update_n_clob_reader_with_length:6:some:-17",
            "result_set_update_n_clob_reader_by_label_with_length:n_clob_long:none:19",
        ]
    );
    assert_eq!(blob.read_to_end().unwrap(), [2, 3]);
    assert_eq!(clob.read_to_string().unwrap(), "乙");
    assert_eq!(n_clob.read_to_string().unwrap(), "丁");

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误只在原 Statement 分类一次，
    // 并且短路后不触达真实只读 RowSet。
    assert_eq!(
        result_set.update_n_clob_reader_by_label_with_length(&mut connection, "fail", None, -23,),
        Err(DruidError::DriverError(
            "filtered LOB stream update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();

    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        94,
        "sqlite-lob-stream-update-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT X'010203' AS value")
        .await
        .unwrap();
    let blob = JdbcInputStream::from_bytes([4, 5, 6]);
    let clob = JdbcReader::from_string("戊己");
    let n_clob = JdbcReader::from_string("庚辛");

    macro_rules! assert_lob_stream_unsupported {
        ($expression:expr, $operation:literal) => {
            assert_eq!(
                $expression,
                Err(DruidError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite RowSet 对 12 个精确入口返回
    // 对应物理 SPI capability error，且每次失败只计数一次。
    assert_lob_stream_unsupported!(
        result_set.update_blob_stream(&mut connection, 1, Some(&blob)),
        "result_set_update_blob_stream"
    );
    assert_lob_stream_unsupported!(
        result_set.update_blob_stream_by_label(&mut connection, "blob", None),
        "result_set_update_blob_stream_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_blob_stream_with_length(&mut connection, 2, Some(&blob), -7),
        "result_set_update_blob_stream"
    );
    assert_lob_stream_unsupported!(
        result_set.update_blob_stream_by_label_with_length(
            &mut connection,
            "blob_long",
            None,
            i64::MAX,
        ),
        "result_set_update_blob_stream_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_clob_reader(&mut connection, 3, Some(&clob)),
        "result_set_update_clob_reader"
    );
    assert_lob_stream_unsupported!(
        result_set.update_clob_reader_by_label(&mut connection, "clob", None),
        "result_set_update_clob_reader_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_clob_reader_with_length(&mut connection, 4, Some(&clob), -11),
        "result_set_update_clob_reader"
    );
    assert_lob_stream_unsupported!(
        result_set.update_clob_reader_by_label_with_length(&mut connection, "clob_long", None, 13,),
        "result_set_update_clob_reader_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_n_clob_reader(&mut connection, 5, Some(&n_clob)),
        "result_set_update_n_clob_reader"
    );
    assert_lob_stream_unsupported!(
        result_set.update_n_clob_reader_by_label(&mut connection, "n_clob", None),
        "result_set_update_n_clob_reader_by_label"
    );
    assert_lob_stream_unsupported!(
        result_set.update_n_clob_reader_with_length(&mut connection, 6, Some(&n_clob), -17),
        "result_set_update_n_clob_reader"
    );
    assert_lob_stream_unsupported!(
        result_set.update_n_clob_reader_by_label_with_length(
            &mut connection,
            "n_clob_long",
            None,
            19,
        ),
        "result_set_update_n_clob_reader_by_label"
    );
    assert_eq!(statement.exception_count(), 12);
    assert!(!result_set.is_closed());
    result_set.close_with_connection(&mut connection).unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn result_set_stream_update_filter_defaults_preserve_all_twenty_two_overloads() {
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&physical_log), false);
    let context = ResultSetFilterContext::new();
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let ascii = JdbcInputStream::from_bytes([1, 2, 3]);
    let binary = JdbcInputStream::from_bytes([4, 5, 6]);
    let character = JdbcReader::from_string("甲乙");
    let n_character = JdbcReader::from_string("丙丁");

    // SOURCE_PARITY / V2_MIRRORED：Java Filter/FilterChainImpl 的 ASCII、
    // Binary、Character × index/label × unspecified/int/long，以及
    // NCharacter × index/label × unspecified/long，共 22 个重载逐一穿透。
    filter_chain
        .result_set_update_ascii_stream(&physical, &context, 1, Some(ascii.clone()))
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_by_label(&physical, &context, "ascii", None)
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_with_int_length(
            &physical,
            &context,
            2,
            Some(ascii.clone()),
            -7,
        )
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_by_label_with_int_length(
            &physical,
            &context,
            "ascii_int",
            None,
            i32::MAX,
        )
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_with_length(
            &physical,
            &context,
            3,
            Some(ascii.clone()),
            -11,
        )
        .unwrap();
    filter_chain
        .result_set_update_ascii_stream_by_label_with_length(
            &physical,
            &context,
            "ascii_long",
            None,
            i64::MAX,
        )
        .unwrap();
    filter_chain
        .result_set_update_binary_stream(&physical, &context, 4, Some(binary.clone()))
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_by_label(&physical, &context, "binary", None)
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_with_int_length(
            &physical,
            &context,
            5,
            Some(binary.clone()),
            i32::MIN,
        )
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_by_label_with_int_length(
            &physical,
            &context,
            "binary_int",
            None,
            13,
        )
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_with_length(
            &physical,
            &context,
            6,
            Some(binary.clone()),
            -17,
        )
        .unwrap();
    filter_chain
        .result_set_update_binary_stream_by_label_with_length(
            &physical,
            &context,
            "binary_long",
            None,
            19,
        )
        .unwrap();
    filter_chain
        .result_set_update_character_stream(&physical, &context, 7, Some(character.clone()))
        .unwrap();
    filter_chain
        .result_set_update_character_stream_by_label(&physical, &context, "character", None)
        .unwrap();
    filter_chain
        .result_set_update_character_stream_with_int_length(
            &physical,
            &context,
            8,
            Some(character.clone()),
            -23,
        )
        .unwrap();
    filter_chain
        .result_set_update_character_stream_by_label_with_int_length(
            &physical,
            &context,
            "character_int",
            None,
            29,
        )
        .unwrap();
    filter_chain
        .result_set_update_character_stream_with_length(
            &physical,
            &context,
            9,
            Some(character.clone()),
            -31,
        )
        .unwrap();
    filter_chain
        .result_set_update_character_stream_by_label_with_length(
            &physical,
            &context,
            "character_long",
            None,
            37,
        )
        .unwrap();
    filter_chain
        .result_set_update_n_character_stream(&physical, &context, 10, Some(n_character.clone()))
        .unwrap();
    filter_chain
        .result_set_update_n_character_stream_by_label(&physical, &context, "n_character", None)
        .unwrap();
    filter_chain
        .result_set_update_n_character_stream_with_length(
            &physical,
            &context,
            11,
            Some(n_character.clone()),
            -41,
        )
        .unwrap();
    filter_chain
        .result_set_update_n_character_stream_by_label_with_length(
            &physical,
            &context,
            "n_character_long",
            None,
            43,
        )
        .unwrap();

    assert_eq!(
        calls(&physical_log),
        [
            "physical:update_value:1:AsciiStream:true:Unspecified",
            "physical:update_value_by_label:ascii:AsciiStream:false:Unspecified",
            "physical:update_value:2:AsciiStream:true:Int(-7)",
            "physical:update_value_by_label:ascii_int:AsciiStream:false:Int(2147483647)",
            "physical:update_value:3:AsciiStream:true:Long(-11)",
            "physical:update_value_by_label:ascii_long:AsciiStream:false:Long(9223372036854775807)",
            "physical:update_value:4:BinaryStream:true:Unspecified",
            "physical:update_value_by_label:binary:BinaryStream:false:Unspecified",
            "physical:update_value:5:BinaryStream:true:Int(-2147483648)",
            "physical:update_value_by_label:binary_int:BinaryStream:false:Int(13)",
            "physical:update_value:6:BinaryStream:true:Long(-17)",
            "physical:update_value_by_label:binary_long:BinaryStream:false:Long(19)",
            "physical:update_value:7:CharacterStream:true:Unspecified",
            "physical:update_value_by_label:character:CharacterStream:false:Unspecified",
            "physical:update_value:8:CharacterStream:true:Int(-23)",
            "physical:update_value_by_label:character_int:CharacterStream:false:Int(29)",
            "physical:update_value:9:CharacterStream:true:Long(-31)",
            "physical:update_value_by_label:character_long:CharacterStream:false:Long(37)",
            "physical:update_value:10:NCharacterStream:true:Unspecified",
            "physical:update_value_by_label:n_character:NCharacterStream:false:Unspecified",
            "physical:update_value:11:NCharacterStream:true:Long(-41)",
            "physical:update_value_by_label:n_character_long:NCharacterStream:false:Long(43)",
        ]
    );

    // RUST_OBLIGATION / V1_RUST_LOCAL：默认 Filter 只克隆共享句柄，不预读、
    // 不关闭，也不校正负长度、i32 边界或超大 long。
    assert_eq!(ascii.read_to_end().unwrap(), [1, 2, 3]);
    assert_eq!(binary.read_to_end().unwrap(), [4, 5, 6]);
    assert_eq!(character.read_to_string().unwrap(), "甲乙");
    assert_eq!(n_character.read_to_string().unwrap(), "丙丁");
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn pooled_result_set_routes_stream_updates_and_preserves_sqlite_capability_errors() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StreamUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        95,
        "sqlite-stream-update-short-circuit".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT X'010203' AS value")
        .await
        .unwrap();
    let ascii = JdbcInputStream::from_bytes([1, 2, 3]);
    let binary = JdbcInputStream::from_bytes([4, 5, 6]);
    let character = JdbcReader::from_string("甲乙");
    let n_character = JdbcReader::from_string("丙丁");

    result_set
        .update_ascii_stream(&mut connection, 1, Some(&ascii))
        .unwrap();
    result_set
        .update_ascii_stream_by_label(&mut connection, "ascii", None)
        .unwrap();
    result_set
        .update_ascii_stream_with_int_length(&mut connection, 2, Some(&ascii), -7)
        .unwrap();
    result_set
        .update_ascii_stream_by_label_with_int_length(&mut connection, "ascii_int", None, i32::MAX)
        .unwrap();
    result_set
        .update_ascii_stream_with_length(&mut connection, 3, Some(&ascii), -11)
        .unwrap();
    result_set
        .update_ascii_stream_by_label_with_length(&mut connection, "ascii_long", None, i64::MAX)
        .unwrap();
    result_set
        .update_binary_stream(&mut connection, 4, Some(&binary))
        .unwrap();
    result_set
        .update_binary_stream_by_label(&mut connection, "binary", None)
        .unwrap();
    result_set
        .update_binary_stream_with_int_length(&mut connection, 5, Some(&binary), i32::MIN)
        .unwrap();
    result_set
        .update_binary_stream_by_label_with_int_length(&mut connection, "binary_int", None, 13)
        .unwrap();
    result_set
        .update_binary_stream_with_length(&mut connection, 6, Some(&binary), -17)
        .unwrap();
    result_set
        .update_binary_stream_by_label_with_length(&mut connection, "binary_long", None, 19)
        .unwrap();
    result_set
        .update_character_stream(&mut connection, 7, Some(&character))
        .unwrap();
    result_set
        .update_character_stream_by_label(&mut connection, "character", None)
        .unwrap();
    result_set
        .update_character_stream_with_int_length(&mut connection, 8, Some(&character), -23)
        .unwrap();
    result_set
        .update_character_stream_by_label_with_int_length(
            &mut connection,
            "character_int",
            None,
            29,
        )
        .unwrap();
    result_set
        .update_character_stream_with_length(&mut connection, 9, Some(&character), -31)
        .unwrap();
    result_set
        .update_character_stream_by_label_with_length(&mut connection, "character_long", None, 37)
        .unwrap();
    result_set
        .update_n_character_stream(&mut connection, 10, Some(&n_character))
        .unwrap();
    result_set
        .update_n_character_stream_by_label(&mut connection, "n_character", None)
        .unwrap();
    result_set
        .update_n_character_stream_with_length(&mut connection, 11, Some(&n_character), -41)
        .unwrap();
    result_set
        .update_n_character_stream_by_label_with_length(
            &mut connection,
            "n_character_long",
            None,
            43,
        )
        .unwrap();

    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_ascii_stream:1:some",
            "result_set_update_ascii_stream_by_label:ascii:none",
            "result_set_update_ascii_stream_with_int_length:2:some:-7",
            "result_set_update_ascii_stream_by_label_with_int_length:ascii_int:none:2147483647",
            "result_set_update_ascii_stream_with_length:3:some:-11",
            "result_set_update_ascii_stream_by_label_with_length:ascii_long:none:9223372036854775807",
            "result_set_update_binary_stream:4:some",
            "result_set_update_binary_stream_by_label:binary:none",
            "result_set_update_binary_stream_with_int_length:5:some:-2147483648",
            "result_set_update_binary_stream_by_label_with_int_length:binary_int:none:13",
            "result_set_update_binary_stream_with_length:6:some:-17",
            "result_set_update_binary_stream_by_label_with_length:binary_long:none:19",
            "result_set_update_character_stream:7:some",
            "result_set_update_character_stream_by_label:character:none",
            "result_set_update_character_stream_with_int_length:8:some:-23",
            "result_set_update_character_stream_by_label_with_int_length:character_int:none:29",
            "result_set_update_character_stream_with_length:9:some:-31",
            "result_set_update_character_stream_by_label_with_length:character_long:none:37",
            "result_set_update_n_character_stream:10:some",
            "result_set_update_n_character_stream_by_label:n_character:none",
            "result_set_update_n_character_stream_with_length:11:some:-41",
            "result_set_update_n_character_stream_by_label_with_length:n_character_long:none:43",
        ]
    );
    assert_eq!(ascii.read_to_end().unwrap(), [2, 3]);
    assert_eq!(binary.read_to_end().unwrap(), [5, 6]);
    assert_eq!(character.read_to_string().unwrap(), "乙");
    assert_eq!(n_character.read_to_string().unwrap(), "丁");

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 错误只在原 Statement 分类一次，
    // 短路后不触达真实只读 RowSet。
    assert_eq!(
        result_set.update_n_character_stream_by_label_with_length(
            &mut connection,
            "fail",
            None,
            -47,
        ),
        Err(DruidError::DriverError(
            "filtered stream update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();

    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        96,
        "sqlite-stream-update-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT X'010203' AS value")
        .await
        .unwrap();

    macro_rules! assert_stream_unsupported {
        ($expression:expr, $operation:literal) => {
            assert_eq!(
                $expression,
                Err(DruidError::UnsupportedOperation {
                    operation: $operation,
                })
            );
        };
    }

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite RowSet 对 22 个精确 Filter
    // 入口保持 generic update capability error，且每次失败只计数一次。
    assert_stream_unsupported!(
        result_set.update_ascii_stream(&mut connection, 1, Some(&ascii)),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_ascii_stream_by_label(&mut connection, "ascii", None),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_ascii_stream_with_int_length(&mut connection, 2, Some(&ascii), -7),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_ascii_stream_by_label_with_int_length(
            &mut connection,
            "ascii_int",
            None,
            i32::MAX,
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_ascii_stream_with_length(&mut connection, 3, Some(&ascii), -11),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_ascii_stream_by_label_with_length(
            &mut connection,
            "ascii_long",
            None,
            i64::MAX,
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_binary_stream(&mut connection, 4, Some(&binary)),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_binary_stream_by_label(&mut connection, "binary", None),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_binary_stream_with_int_length(
            &mut connection,
            5,
            Some(&binary),
            i32::MIN,
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_binary_stream_by_label_with_int_length(
            &mut connection,
            "binary_int",
            None,
            13,
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_binary_stream_with_length(&mut connection, 6, Some(&binary), -17),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_binary_stream_by_label_with_length(
            &mut connection,
            "binary_long",
            None,
            19,
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_character_stream(&mut connection, 7, Some(&character)),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_character_stream_by_label(&mut connection, "character", None),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_character_stream_with_int_length(
            &mut connection,
            8,
            Some(&character),
            -23,
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_character_stream_by_label_with_int_length(
            &mut connection,
            "character_int",
            None,
            29,
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_character_stream_with_length(&mut connection, 9, Some(&character), -31,),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_character_stream_by_label_with_length(
            &mut connection,
            "character_long",
            None,
            37,
        ),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_n_character_stream(&mut connection, 10, Some(&n_character)),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_n_character_stream_by_label(&mut connection, "n_character", None),
        "result_set_update_value_by_label"
    );
    assert_stream_unsupported!(
        result_set.update_n_character_stream_with_length(
            &mut connection,
            11,
            Some(&n_character),
            -41,
        ),
        "result_set_update_value"
    );
    assert_stream_unsupported!(
        result_set.update_n_character_stream_by_label_with_length(
            &mut connection,
            "n_character_long",
            None,
            43,
        ),
        "result_set_update_value_by_label"
    );
    assert_eq!(statement.exception_count(), 22);
    assert!(!result_set.is_closed());
    result_set.close_with_connection(&mut connection).unwrap();
}

#[test]
fn result_set_n_string_update_filter_defaults_preserve_both_overloads() {
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let physical = PhysicalResultSetProbe::new(Arc::clone(&physical_log), false);
    let context = ResultSetFilterContext::new();
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(PassThroughResultSetFilter));

    // SOURCE_PARITY / V2_MIRRORED：Java Filter/FilterChainImpl 的
    // updateNString index/label 两条签名逐一穿透，Java null 保持为 None。
    filter_chain
        .result_set_update_n_string(&physical, &context, 1, Some("国语".to_string()))
        .unwrap();
    filter_chain
        .result_set_update_n_string_by_label(&physical, &context, "名称", None)
        .unwrap();

    assert_eq!(
        calls(&physical_log),
        [
            "physical:update_value:1:NString(Some(\"国语\"))",
            "physical:update_value_by_label:名称:NString(None)",
        ]
    );
}

#[tokio::test]
async fn pooled_result_set_routes_n_string_updates_and_preserves_sqlite_capability_errors() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(NStringUpdateShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        97,
        "sqlite-n-string-update-short-circuit".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT '原值' AS value")
        .await
        .unwrap();

    // RUST_OBLIGATION / V1_RUST_LOCAL：NString 不得折叠为普通 String Filter，
    // Unicode、nullable 与 label 身份进入独立两条方法。
    result_set
        .update_n_string(&mut connection, 1, Some("过滤-索引".to_string()))
        .unwrap();
    result_set
        .update_n_string_by_label(&mut connection, "名称", None)
        .unwrap();
    assert_eq!(
        calls(&call_log),
        [
            "result_set_update_n_string:1:Some(\"过滤-索引\")",
            "result_set_update_n_string_by_label:名称:None",
        ]
    );
    assert_eq!(
        result_set.update_n_string_by_label(&mut connection, "fail", Some("失败".to_string()),),
        Err(DruidError::DriverError(
            "filtered NString update failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();

    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        98,
        "sqlite-n-string-update-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT '原值' AS value")
        .await
        .unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite 只读 RowSet 的 generic update
    // capability error 必须穿过两条精确 Filter 入口，并各计数一次。
    assert_eq!(
        result_set.update_n_string(&mut connection, 1, Some("索引".to_string())),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value",
        })
    );
    assert_eq!(
        result_set.update_n_string_by_label(&mut connection, "value", None),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_update_value_by_label",
        })
    );
    assert_eq!(statement.exception_count(), 2);
    assert!(!result_set.is_closed());
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn pooled_result_set_preserves_distinct_n_string_filter_overloads() {
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let physical_probe = PhysicalResultSetProbe::new(Arc::clone(&physical_log), false);
    let context = ResultSetFilterContext::new();
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    assert_eq!(
        pass_chain
            .result_set_get_n_string(&physical_probe, &context, 1)
            .unwrap(),
        Some("索引-NString".to_string())
    );
    assert_eq!(
        pass_chain
            .result_set_get_n_string_by_label(&physical_probe, &context, "name")
            .unwrap(),
        Some("标签-NString".to_string())
    );
    assert_eq!(
        calls(&physical_log),
        ["physical:n_string:1", "physical:n_string_by_label:name"]
    );

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(NStringShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        80,
        "sqlite-n-string-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT '原值'")
        .await
        .unwrap();
    assert_eq!(
        result_set.n_string(&mut connection, 3).unwrap(),
        Some("过滤-索引".to_string())
    );
    assert_eq!(
        result_set
            .n_string_by_label(&mut connection, "name")
            .unwrap(),
        Some("过滤-标签".to_string())
    );
    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_n_string:3",
            "result_set_get_n_string_by_label:name"
        ]
    );
    assert_eq!(
        result_set.n_string_by_label(&mut connection, "fail"),
        Err(DruidError::DriverError(
            "filtered NString failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite Unicode 值经独立 NString Filter 入口读取。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        81,
        "sqlite-n-string-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT '你好，Druid'")
        .await
        .unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.n_string(&mut connection, 1).unwrap(),
        Some("你好，Druid".to_string())
    );
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn pooled_result_set_routes_metadata_platform_handle_through_filter() {
    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let physical_probe = PhysicalResultSetProbe::new(Arc::clone(&physical_log), false);
    let context = ResultSetFilterContext::new();
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    assert_eq!(
        pass_chain
            .result_set_get_meta_data(&physical_probe, &context)
            .unwrap()
            .column_count()
            .unwrap(),
        0
    );
    assert_eq!(calls(&physical_log), ["physical:meta_data"]);

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let fail = Arc::new(AtomicBool::new(false));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(MetadataShortCircuitFilter {
        calls: Arc::clone(&call_log),
        fail: Arc::clone(&fail),
    }));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        82,
        "sqlite-metadata-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1")
        .await
        .unwrap();
    assert_eq!(
        result_set
            .meta_data(&mut connection)
            .unwrap()
            .column_count()
            .unwrap(),
        0
    );
    fail.store(true, Ordering::Release);
    assert_eq!(
        result_set.meta_data(&mut connection).map(|_| ()),
        Err(DruidError::DriverError(
            "filtered metadata failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(
        calls(&call_log),
        ["result_set_get_meta_data", "result_set_get_meta_data"]
    );
    result_set.close_with_connection(&mut connection).unwrap();

    // VALUE_ADD / V5_HOST：真实 Toasty SQLite metadata 经默认 Filter 返回平台句柄。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(PassThroughResultSetFilter));
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        83,
        "sqlite-metadata-pass-through".to_string(),
        Some(Arc::new(pass_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1, 'two'")
        .await
        .unwrap();
    let metadata = result_set.meta_data(&mut connection).unwrap();
    assert_eq!(metadata.column_count().unwrap(), 2);
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn pooled_result_set_routes_dynamic_statement_identity_through_filter() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let replacement = Arc::new(Mutex::new(None));
    let fail = Arc::new(AtomicBool::new(false));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StatementShortCircuitFilter {
        calls: Arc::clone(&call_log),
        replacement: Arc::clone(&replacement),
        fail: Arc::clone(&fail),
    }));
    filter_chain.add_result_set(Arc::new(PassThroughResultSetFilter));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::with_context(
        physical,
        84,
        "sqlite-statement-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let replacement_statement = connection.create_statement().await.unwrap();
    let mut source_statement = connection.create_statement().await.unwrap();
    let mut result_set = source_statement
        .execute_query_result_set(&mut connection, "SELECT 1")
        .await
        .unwrap();

    // SOURCE_PARITY / V2_MIRRORED：默认继续链必须返回创建结果集的同一逻辑对象。
    let returned = result_set.statement_object(&mut connection).unwrap();
    assert!(matches!(returned, ResultSetStatement::Statement(_)));
    assert!(returned
        .pooled_statement()
        .is_same_statement(&source_statement));
    assert!(returned.prepared_statement().is_none());
    assert!(returned.callable_statement().is_none());
    assert!(!returned.is_closed());

    // RUST_OBLIGATION / V1_RUST_LOCAL：Filter 可用拥有型共享句柄短路替换返回对象。
    replacement
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .replace(ResultSetStatement::Statement(replacement_statement.clone()));
    let returned = result_set.statement_object(&mut connection).unwrap();
    assert!(returned
        .pooled_statement()
        .is_same_statement(&replacement_statement));
    assert!(!returned
        .pooled_statement()
        .is_same_statement(&source_statement));
    assert!(returned.prepared_statement().is_none());
    assert!(returned.callable_statement().is_none());
    assert!(!returned.is_closed());

    // SOURCE_PARITY / V2_MIRRORED：Filter 驱动错误归入创建结果集的 Statement。
    fail.store(true, Ordering::Release);
    assert_eq!(
        result_set.statement_object(&mut connection).map(|_| ()),
        Err(DruidError::DriverError(
            "filtered statement failure".to_string()
        ))
    );
    assert_eq!(source_statement.exception_count(), 1);
    assert_eq!(
        calls(&call_log),
        [
            "result_set_get_statement",
            "result_set_get_statement",
            "result_set_get_statement"
        ]
    );

    // VALUE_ADD / V5_HOST：上述路径使用真实 Toasty SQLite ResultSet。
    fail.store(false, Ordering::Release);
    result_set.close_with_connection(&mut connection).unwrap();
}

#[tokio::test]
async fn real_sqlite_result_sets_flow_through_stat_filter_and_statement_cascade() {
    let collector = Arc::new(StatsCollector::new("sqlite-stat", Duration::from_secs(1)));
    let stat_filter = Arc::new(StatFilter::new(Arc::clone(&collector)));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_filter(stat_filter);

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        72,
        "sqlite-stat".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );

    let mut first_statement = connection.create_statement().await.unwrap();
    let mut first = first_statement
        .execute_query_result_set(
            &mut connection,
            "SELECT 1 AS n, '12.340' AS decimal_value, '2025-01-02' AS date_value, \
             '03:04:05' AS time_value, '2025-01-02 03:04:05' AS timestamp_value \
             UNION ALL SELECT 2, '56.780', '2025-02-03', '06:07:08', \
             '2025-02-03 06:07:08' UNION ALL SELECT 3, '90.120', '2025-03-04', \
             '09:10:11', '2025-03-04 09:10:11'",
        )
        .await
        .unwrap();
    assert_eq!(collector.result_set_stat().opening_count(), 1);
    assert!(first.next(&mut connection).unwrap());
    assert_eq!(first.int(&mut connection, 1).unwrap(), 1);
    assert_eq!(first.object(&mut connection, 1).unwrap(), Value::Int(1));
    assert_eq!(
        first
            .object_typed(&mut connection, 1, &JdbcTargetType::Long)
            .unwrap(),
        JdbcObject::Long(1)
    );
    assert_eq!(
        first.big_decimal(&mut connection, 2).unwrap(),
        Some(BigDecimal::from_str("12.340").unwrap())
    );
    assert_eq!(
        first.date(&mut connection, 3).unwrap(),
        Some(NaiveDate::from_ymd_opt(2025, 1, 2).unwrap())
    );
    assert_eq!(
        first.time_with_calendar(&mut connection, 4, None).unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        first
            .timestamp_with_calendar(&mut connection, 5, Some(JdbcCalendar::new("UTC").unwrap()),)
            .unwrap(),
        Some(
            NaiveDate::from_ymd_opt(2025, 1, 2)
                .unwrap()
                .and_hms_opt(3, 4, 5)
                .unwrap()
        )
    );
    assert!(first.next(&mut connection).unwrap());
    first.close_with_connection(&mut connection).unwrap();
    assert_eq!(collector.result_set_stat().opening_count(), 0);
    assert_eq!(collector.result_set_stat().fetch_row_count(), 2);
    assert_eq!(collector.result_set_stat().close_count(), 1);

    first_statement
        .close_with_connection(&mut connection)
        .unwrap();
    assert_eq!(collector.result_set_stat().close_count(), 1);

    let mut second_statement = connection.create_statement().await.unwrap();
    let mut second = second_statement
        .execute_query_result_set(&mut connection, "SELECT 4 UNION ALL SELECT 5")
        .await
        .unwrap();
    assert!(second.next(&mut connection).unwrap());
    second_statement
        .close_with_connection(&mut connection)
        .unwrap();

    let stat = collector.result_set_stat();
    assert_eq!(stat.open_count(), 2);
    assert_eq!(stat.opening_count(), 0);
    assert_eq!(stat.opening_max(), 1);
    assert_eq!(stat.fetch_row_count(), 3);
    assert_eq!(stat.close_count(), 2);
    assert!(stat.last_open_time_millis().is_some());
    assert!(second.is_closed());
    assert!(second.raw_result_set().is_closed());
}
