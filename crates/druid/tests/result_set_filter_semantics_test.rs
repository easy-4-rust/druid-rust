//! Java `ResultSet FilterChain` 与 `StatFilter` 的顺序、短路及真实 `SQLite` 契约。

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    DruidError, DruidPooledConnection, FilterChain, JdbcArray, JdbcBlob, JdbcCalendar,
    JdbcCalendarArgument, JdbcClob, JdbcInputStream, JdbcNClob, JdbcObject, JdbcReader, JdbcRef,
    JdbcRowId, JdbcSqlXml, JdbcTargetType, JdbcTypeMap, JdbcUrl, PhysicalConnectionFactory,
    PhysicalResultSet, ResultSetFilter, ResultSetFilterChain, ResultSetFilterContext, Value,
};
use druid::stats::{StatFilter, StatsCollector};
use druid::toasty::ToastyConnectionFactory;
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
