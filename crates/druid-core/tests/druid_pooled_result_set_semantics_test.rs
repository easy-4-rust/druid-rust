//! `DruidPooledResultSet` 的 Java 对照与真实 `SQLite` 契约测试。

extern crate druid_core as druid;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_core::core::{
    DruidError, DruidPooledConnection, DruidPooledResultSet, PhysicalConnection,
    PhysicalConnectionFactory, PhysicalRdbcOpaqueObject, PhysicalResultSet,
    PhysicalResultSetMetaData, RdbcArray, RdbcBlob, RdbcCalendar, RdbcCalendarArgument,
    RdbcCharacterLength, RdbcClob, RdbcInputStream, RdbcNClob, RdbcObject, RdbcOpaqueObject,
    RdbcReader, RdbcRef, RdbcRowId, RdbcSqlXml, RdbcStreamLength, RdbcTargetType, RdbcTypeMap,
    RdbcUrl, ResultSetColumnMeta, ResultSetColumnType, ResultSetMetaData, ResultSetNullability,
    ResultSetUpdate, Row, RowSetResultSet, SqlWarning, Value, Wrapper, WrapperExt,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::{Any, TypeId};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, PartialEq, Eq)]
struct VendorObjectProbe {
    id: i32,
}

impl PhysicalRdbcOpaqueObject for VendorObjectProbe {
    fn class_name(&self) -> &'static str {
        "com.example.VendorObject"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct CustomTargetReadProbe {
    value_calls: AtomicUsize,
}

impl PhysicalResultSet for CustomTargetReadProbe {
    fn close(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn value(&self, column_index: usize) -> Result<Value, DruidError> {
        assert_eq!(column_index, 3);
        self.value_calls.fetch_add(1, Ordering::Relaxed);
        Ok(Value::String("vendor-value".to_string()))
    }
}

#[test]
fn custom_typed_object_checks_the_physical_column_exactly_once() {
    let probe = CustomTargetReadProbe {
        value_calls: AtomicUsize::new(0),
    };
    assert!(matches!(
        probe.object_as(3, &RdbcTargetType::Custom("vendor.Type".to_string())),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_get_object_typed_custom"
        })
    ));
    assert_eq!(probe.value_calls.load(Ordering::Relaxed), 1);
}

#[derive(Debug)]
struct ScalarGetterDelegationProbe {
    calls: Mutex<Vec<&'static str>>,
    fail_on: Option<&'static str>,
}

impl ScalarGetterDelegationProbe {
    fn record<T>(&self, method: &'static str, value: T) -> Result<T, DruidError> {
        self.calls.lock().unwrap().push(method);
        if self.fail_on == Some(method) {
            Err(DruidError::DriverError(format!("{method} failed")))
        } else {
            Ok(value)
        }
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl PhysicalResultSet for ScalarGetterDelegationProbe {
    fn close(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn value_by_label(&self, _column_label: &str) -> Result<Value, DruidError> {
        self.record("value_by_label", Value::Int(90))
    }

    fn string(&self, _column_index: usize) -> Result<Option<String>, DruidError> {
        self.record("string", Some("index".to_string()))
    }

    fn string_by_label(&self, _column_label: &str) -> Result<Option<String>, DruidError> {
        self.record("string_by_label", Some("label".to_string()))
    }

    fn boolean(&self, _column_index: usize) -> Result<bool, DruidError> {
        self.record("boolean", true)
    }

    fn boolean_by_label(&self, _column_label: &str) -> Result<bool, DruidError> {
        self.record("boolean_by_label", false)
    }

    fn long(&self, _column_index: usize) -> Result<i64, DruidError> {
        self.record("long", 101)
    }

    fn long_by_label(&self, _column_label: &str) -> Result<i64, DruidError> {
        self.record("long_by_label", 102)
    }

    fn int(&self, _column_index: usize) -> Result<i32, DruidError> {
        self.record("int", 103)
    }

    fn int_by_label(&self, _column_label: &str) -> Result<i32, DruidError> {
        self.record("int_by_label", 104)
    }

    fn short(&self, _column_index: usize) -> Result<i16, DruidError> {
        self.record("short", 105)
    }

    fn short_by_label(&self, _column_label: &str) -> Result<i16, DruidError> {
        self.record("short_by_label", 106)
    }

    fn byte(&self, _column_index: usize) -> Result<i8, DruidError> {
        self.record("byte", 107)
    }

    fn byte_by_label(&self, _column_label: &str) -> Result<i8, DruidError> {
        self.record("byte_by_label", 108)
    }

    fn double(&self, _column_index: usize) -> Result<f64, DruidError> {
        self.record("double", 109.5)
    }

    fn double_by_label(&self, _column_label: &str) -> Result<f64, DruidError> {
        self.record("double_by_label", 110.5)
    }

    fn float(&self, _column_index: usize) -> Result<f32, DruidError> {
        self.record("float", 111.5)
    }

    fn float_by_label(&self, _column_label: &str) -> Result<f32, DruidError> {
        self.record("float_by_label", 112.5)
    }

    fn bytes(&self, _column_index: usize) -> Result<Option<Vec<u8>>, DruidError> {
        self.record("bytes", Some(vec![113]))
    }

    fn bytes_by_label(&self, _column_label: &str) -> Result<Option<Vec<u8>>, DruidError> {
        self.record("bytes_by_label", Some(vec![114]))
    }
}

fn assert_scalar_getter_delegation(
    result_set: &mut DruidPooledResultSet,
    connection: &mut DruidPooledConnection,
) {
    assert_eq!(
        result_set.object_by_label(connection, "object").unwrap(),
        Value::Int(90)
    );
    assert_eq!(
        result_set.string(connection, 1).unwrap().as_deref(),
        Some("index")
    );
    assert_eq!(
        result_set
            .string_by_label(connection, "string")
            .unwrap()
            .as_deref(),
        Some("label")
    );
    assert!(result_set.boolean(connection, 2).unwrap());
    assert!(!result_set.boolean_by_label(connection, "boolean").unwrap());
    assert_eq!(result_set.long(connection, 3).unwrap(), 101);
    assert_eq!(result_set.long_by_label(connection, "long").unwrap(), 102);
    assert_eq!(result_set.int(connection, 4).unwrap(), 103);
    assert_eq!(result_set.int_by_label(connection, "int").unwrap(), 104);
    assert_eq!(result_set.short(connection, 5).unwrap(), 105);
    assert_eq!(result_set.short_by_label(connection, "short").unwrap(), 106);
    assert_eq!(result_set.byte(connection, 6).unwrap(), 107);
    assert_eq!(result_set.byte_by_label(connection, "byte").unwrap(), 108);
    assert_eq!(result_set.double(connection, 7).unwrap(), 109.5);
    assert_eq!(
        result_set.double_by_label(connection, "double").unwrap(),
        110.5
    );
    assert_eq!(result_set.float(connection, 8).unwrap(), 111.5);
    assert_eq!(
        result_set.float_by_label(connection, "float").unwrap(),
        112.5
    );
    assert_eq!(result_set.bytes(connection, 9).unwrap(), Some(vec![113]));
    assert_eq!(
        result_set.bytes_by_label(connection, "bytes").unwrap(),
        Some(vec![114])
    );
}

#[tokio::test]
async fn pooled_scalar_getters_delegate_exact_physical_overloads_and_classify_errors() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let probe = Arc::new(ScalarGetterDelegationProbe {
        calls: Mutex::new(Vec::new()),
        fail_on: Some("boolean_by_label"),
    });
    let physical: Arc<dyn PhysicalResultSet> = probe.clone();
    let mut result_set = statement.wrap_result_set(physical).unwrap();

    assert!(matches!(
        result_set.boolean_by_label(&mut connection, "failing"),
        Err(DruidError::DriverError(message)) if message == "boolean_by_label failed"
    ));
    assert_eq!(statement.exception_count(), 1);

    probe.calls.lock().unwrap().clear();
    let successful_probe = Arc::new(ScalarGetterDelegationProbe {
        calls: Mutex::new(Vec::new()),
        fail_on: None,
    });
    let physical: Arc<dyn PhysicalResultSet> = successful_probe.clone();
    let mut successful_result_set = statement.wrap_result_set(physical).unwrap();
    assert_scalar_getter_delegation(&mut successful_result_set, &mut connection);
    assert_eq!(successful_probe.calls().len(), 19);
}

#[derive(Debug)]
struct SparsePhysicalResultSet;

impl PhysicalResultSet for SparsePhysicalResultSet {
    fn close(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn is_closed(&self) -> bool {
        false
    }
}

fn assert_unsupported_operation<T: std::fmt::Debug>(
    result: Result<T, DruidError>,
    expected_operation: &'static str,
) {
    assert!(
        matches!(
            result,
            Err(DruidError::UnsupportedOperation { operation })
                if operation == expected_operation
        ),
        "expected unsupported operation {expected_operation}"
    );
}

#[test]
fn sparse_result_set_navigation_defaults_report_exact_capabilities() {
    let result_set = SparsePhysicalResultSet;
    assert_unsupported_operation(result_set.next(), "result_set_next");
    assert_unsupported_operation(result_set.previous(), "result_set_previous");
    assert_unsupported_operation(result_set.first(), "result_set_first");
    assert_unsupported_operation(result_set.last(), "result_set_last");
    assert_unsupported_operation(result_set.before_first(), "result_set_before_first");
    assert_unsupported_operation(result_set.after_last(), "result_set_after_last");
    assert_unsupported_operation(result_set.absolute(-2), "result_set_absolute");
    assert_unsupported_operation(result_set.relative(3), "result_set_relative");
    assert_unsupported_operation(result_set.row(), "result_set_row");
    assert_unsupported_operation(result_set.value(1), "result_set_value");
}

#[test]
fn sparse_result_set_typed_defaults_preserve_error_priority() {
    let result_set = SparsePhysicalResultSet;
    let calendar = RdbcCalendarArgument::Unspecified;
    assert_unsupported_operation(result_set.big_decimal(1, None), "result_set_value");
    assert_unsupported_operation(
        result_set.big_decimal_by_label("amount", Some(2)),
        "result_set_find_column",
    );
    assert_unsupported_operation(result_set.date(1, &calendar), "result_set_value");
    assert_unsupported_operation(
        result_set.date_by_label("date", &calendar),
        "result_set_find_column",
    );
    assert_unsupported_operation(result_set.time(1, &calendar), "result_set_value");
    assert_unsupported_operation(
        result_set.time_by_label("time", &calendar),
        "result_set_find_column",
    );
    assert_unsupported_operation(result_set.timestamp(1, &calendar), "result_set_value");
    assert_unsupported_operation(
        result_set.timestamp_by_label("timestamp", &calendar),
        "result_set_find_column",
    );
    assert_unsupported_operation(
        result_set.object_with_type_map(1, None),
        "result_set_get_object_with_type_map",
    );
    assert_unsupported_operation(
        result_set.object_by_label_with_type_map("object", None),
        "result_set_get_object_by_label_with_type_map",
    );
    assert_unsupported_operation(
        result_set.object_as(1, &RdbcTargetType::Custom("vendor.Type".to_string())),
        "result_set_value",
    );
    assert_unsupported_operation(
        result_set.object_by_label_as("object", &RdbcTargetType::Custom("vendor.Type".to_string())),
        "result_set_find_column",
    );
}

#[test]
fn sparse_result_set_property_defaults_report_exact_capabilities() {
    let result_set = SparsePhysicalResultSet;
    assert_unsupported_operation(result_set.find_column("id"), "result_set_find_column");
    assert_unsupported_operation(result_set.was_null(), "result_set_was_null");
    assert_unsupported_operation(result_set.ascii_stream(1), "result_set_ascii_stream");
    assert_unsupported_operation(result_set.unicode_stream(1), "result_set_unicode_stream");
    assert_unsupported_operation(result_set.binary_stream(1), "result_set_binary_stream");
    assert_unsupported_operation(
        result_set.character_stream(1),
        "result_set_character_stream",
    );
    assert_unsupported_operation(result_set.is_before_first(), "result_set_is_before_first");
    assert_unsupported_operation(result_set.is_after_last(), "result_set_is_after_last");
    assert_unsupported_operation(result_set.is_first(), "result_set_is_first");
    assert_unsupported_operation(result_set.is_last(), "result_set_is_last");
    assert_unsupported_operation(
        result_set.set_fetch_direction(1000),
        "result_set_set_fetch_direction",
    );
    assert_unsupported_operation(result_set.fetch_direction(), "result_set_fetch_direction");
    assert_unsupported_operation(result_set.set_fetch_size(10), "result_set_set_fetch_size");
    assert_unsupported_operation(result_set.fetch_size(), "result_set_fetch_size");
    assert_unsupported_operation(result_set.result_set_type(), "result_set_type");
    assert_unsupported_operation(result_set.concurrency(), "result_set_concurrency");
    assert_unsupported_operation(result_set.holdability(), "result_set_holdability");
    assert_unsupported_operation(result_set.warnings(), "result_set_warnings");
    assert_unsupported_operation(result_set.clear_warnings(), "result_set_clear_warnings");
    assert_unsupported_operation(result_set.cursor_name(), "result_set_cursor_name");
    assert_unsupported_operation(result_set.meta_data(), "result_set_meta_data");
    assert_unsupported_operation(result_set.row_updated(), "result_set_row_updated");
    assert_unsupported_operation(result_set.row_inserted(), "result_set_row_inserted");
    assert_unsupported_operation(result_set.row_deleted(), "result_set_row_deleted");
    assert_unsupported_operation(result_set.insert_row(), "result_set_insert_row");
    assert_unsupported_operation(result_set.update_row(), "result_set_update_row");
    assert_unsupported_operation(result_set.delete_row(), "result_set_delete_row");
    assert_unsupported_operation(result_set.refresh_row(), "result_set_refresh_row");
    assert_unsupported_operation(
        result_set.cancel_row_updates(),
        "result_set_cancel_row_updates",
    );
    assert_unsupported_operation(
        result_set.move_to_insert_row(),
        "result_set_move_to_insert_row",
    );
    assert_unsupported_operation(
        result_set.move_to_current_row(),
        "result_set_move_to_current_row",
    );
}

fn assert_sparse_pooled_navigation_errors(
    result_set: &mut DruidPooledResultSet,
    connection: &mut DruidPooledConnection,
) {
    assert_unsupported_operation(result_set.next(connection), "result_set_next");
    assert_unsupported_operation(result_set.previous(connection), "result_set_previous");
    assert_unsupported_operation(
        result_set.is_before_first(connection),
        "result_set_is_before_first",
    );
    assert_unsupported_operation(
        result_set.is_after_last(connection),
        "result_set_is_after_last",
    );
    assert_unsupported_operation(result_set.is_first(connection), "result_set_is_first");
    assert_unsupported_operation(result_set.is_last(connection), "result_set_is_last");
    assert_unsupported_operation(
        result_set.before_first(connection),
        "result_set_before_first",
    );
    assert_unsupported_operation(result_set.after_last(connection), "result_set_after_last");
    assert_unsupported_operation(result_set.first(connection), "result_set_first");
    assert_unsupported_operation(result_set.last(connection), "result_set_last");
    assert_unsupported_operation(result_set.row(connection), "result_set_row");
    assert_unsupported_operation(result_set.absolute(connection, -2), "result_set_absolute");
    assert_unsupported_operation(result_set.relative(connection, 3), "result_set_relative");
}

fn assert_sparse_pooled_property_errors(
    result_set: &mut DruidPooledResultSet,
    connection: &mut DruidPooledConnection,
) {
    assert_unsupported_operation(
        result_set.set_fetch_direction(connection, 1000),
        "result_set_set_fetch_direction",
    );
    assert_unsupported_operation(
        result_set.fetch_direction(connection),
        "result_set_fetch_direction",
    );
    assert_unsupported_operation(
        result_set.set_fetch_size(connection, 10),
        "result_set_set_fetch_size",
    );
    assert_unsupported_operation(result_set.fetch_size(connection), "result_set_fetch_size");
    assert_unsupported_operation(result_set.result_set_type(connection), "result_set_type");
    assert_unsupported_operation(result_set.concurrency(connection), "result_set_concurrency");
    assert_unsupported_operation(result_set.holdability(connection), "result_set_holdability");
    assert_unsupported_operation(result_set.warnings(connection), "result_set_warnings");
    assert_unsupported_operation(
        result_set.clear_warnings(connection),
        "result_set_clear_warnings",
    );
    assert_unsupported_operation(result_set.cursor_name(connection), "result_set_cursor_name");
    assert_unsupported_operation(result_set.meta_data(connection), "result_set_meta_data");
    assert_unsupported_operation(result_set.row_updated(connection), "result_set_row_updated");
    assert_unsupported_operation(
        result_set.row_inserted(connection),
        "result_set_row_inserted",
    );
    assert_unsupported_operation(result_set.row_deleted(connection), "result_set_row_deleted");
}

fn assert_sparse_pooled_row_mutation_errors(
    result_set: &mut DruidPooledResultSet,
    connection: &mut DruidPooledConnection,
) {
    assert_unsupported_operation(result_set.insert_row(connection), "result_set_insert_row");
    assert_unsupported_operation(result_set.update_row(connection), "result_set_update_row");
    assert_unsupported_operation(result_set.delete_row(connection), "result_set_delete_row");
    assert_unsupported_operation(result_set.refresh_row(connection), "result_set_refresh_row");
    assert_unsupported_operation(
        result_set.cancel_row_updates(connection),
        "result_set_cancel_row_updates",
    );
    assert_unsupported_operation(
        result_set.move_to_insert_row(connection),
        "result_set_move_to_insert_row",
    );
    assert_unsupported_operation(
        result_set.move_to_current_row(connection),
        "result_set_move_to_current_row",
    );
}

#[tokio::test]
async fn pooled_result_set_classifies_every_sparse_capability_at_the_java_call_site() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let physical: Arc<dyn PhysicalResultSet> = Arc::new(SparsePhysicalResultSet);
    let mut result_set = statement.wrap_result_set(physical).unwrap();

    assert_sparse_pooled_navigation_errors(&mut result_set, &mut connection);
    assert_sparse_pooled_property_errors(&mut result_set, &mut connection);
    assert_sparse_pooled_row_mutation_errors(&mut result_set, &mut connection);

    assert_eq!(statement.exception_count(), 34);
    result_set.close_with_connection(&mut connection).unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TypedResourceCall {
    FindColumn(String),
    Blob(usize),
    Clob(usize),
    NClob(usize),
    Array(usize),
    Ref(usize),
    RowId(usize),
    SqlXml(usize),
    Url(usize),
}

#[derive(Debug)]
struct TypedResourceProbe {
    calls: Mutex<Vec<TypedResourceCall>>,
}

impl TypedResourceProbe {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<TypedResourceCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl PhysicalResultSet for TypedResourceProbe {
    fn close(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn find_column(&self, column_label: &str) -> Result<usize, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::FindColumn(column_label.to_string()));
        Ok(41)
    }

    fn blob(&self, column_index: usize) -> Result<Option<RdbcBlob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::Blob(column_index));
        Ok(None)
    }

    fn clob(&self, column_index: usize) -> Result<Option<RdbcClob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::Clob(column_index));
        Ok(None)
    }

    fn n_clob(&self, column_index: usize) -> Result<Option<RdbcNClob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::NClob(column_index));
        Ok(None)
    }

    fn array(&self, column_index: usize) -> Result<Option<RdbcArray>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::Array(column_index));
        Ok(None)
    }

    fn reference(&self, column_index: usize) -> Result<Option<RdbcRef>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::Ref(column_index));
        Ok(None)
    }

    fn row_id(&self, column_index: usize) -> Result<Option<RdbcRowId>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::RowId(column_index));
        Ok(None)
    }

    fn sql_xml(&self, column_index: usize) -> Result<Option<RdbcSqlXml>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::SqlXml(column_index));
        Ok(None)
    }

    fn url(&self, column_index: usize) -> Result<Option<RdbcUrl>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(TypedResourceCall::Url(column_index));
        Ok(None)
    }
}

#[derive(Debug)]
struct PhysicalMetaDataProbe {
    calls: Mutex<Vec<&'static str>>,
    fail_method: Option<&'static str>,
}

impl PhysicalMetaDataProbe {
    fn new(fail_method: Option<&'static str>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_method,
        }
    }

    fn call<T>(&self, method: &'static str, value: T) -> Result<T, DruidError> {
        self.calls.lock().unwrap().push(method);
        if self.fail_method == Some(method) {
            Err(DruidError::DriverError(format!(
                "metadata probe failed at {method}"
            )))
        } else {
            Ok(value)
        }
    }

    fn calls(&self) -> Vec<&'static str> {
        self.calls.lock().unwrap().clone()
    }
}

impl Wrapper for PhysicalMetaDataProbe {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PhysicalResultSetMetaData for PhysicalMetaDataProbe {
    fn column_count(&self) -> Result<usize, DruidError> {
        self.call("column_count", 1)
    }

    fn is_auto_increment(&self, column_index: usize) -> Result<bool, DruidError> {
        self.call("is_auto_increment", column_index == 1)
    }

    fn is_case_sensitive(&self, column_index: usize) -> Result<bool, DruidError> {
        self.call("is_case_sensitive", column_index == 1)
    }

    fn is_searchable(&self, column_index: usize) -> Result<bool, DruidError> {
        self.call("is_searchable", column_index == 1)
    }

    fn is_currency(&self, column_index: usize) -> Result<bool, DruidError> {
        self.call("is_currency", column_index == 1)
    }

    fn nullability(&self, column_index: usize) -> Result<ResultSetNullability, DruidError> {
        self.call(
            "nullability",
            if column_index == 1 {
                ResultSetNullability::Nullable
            } else {
                ResultSetNullability::Unknown
            },
        )
    }

    fn is_signed(&self, column_index: usize) -> Result<bool, DruidError> {
        self.call("is_signed", column_index == 1)
    }

    fn column_display_size(&self, column_index: usize) -> Result<usize, DruidError> {
        self.call("column_display_size", column_index + 20)
    }

    fn column_label(&self, column_index: usize) -> Result<String, DruidError> {
        self.call("column_label", format!("label_{column_index}"))
    }

    fn column_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.call("column_name", format!("name_{column_index}"))
    }

    fn schema_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.call("schema_name", format!("schema_{column_index}"))
    }

    fn precision(&self, column_index: usize) -> Result<usize, DruidError> {
        self.call("precision", column_index + 30)
    }

    fn scale(&self, column_index: usize) -> Result<usize, DruidError> {
        self.call("scale", column_index + 4)
    }

    fn table_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.call("table_name", format!("table_{column_index}"))
    }

    fn catalog_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.call("catalog_name", format!("catalog_{column_index}"))
    }

    fn column_type(&self, column_index: usize) -> Result<ResultSetColumnType, DruidError> {
        self.call(
            "column_type",
            if column_index == 1 {
                ResultSetColumnType::Decimal
            } else {
                ResultSetColumnType::Unknown
            },
        )
    }

    fn column_type_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.call("column_type_name", format!("MONEY_{column_index}"))
    }

    fn is_read_only(&self, column_index: usize) -> Result<bool, DruidError> {
        self.call("is_read_only", column_index != 1)
    }

    fn is_writable(&self, column_index: usize) -> Result<bool, DruidError> {
        self.call("is_writable", column_index == 1)
    }

    fn is_definitely_writable(&self, column_index: usize) -> Result<bool, DruidError> {
        self.call("is_definitely_writable", column_index == 1)
    }

    fn column_class_name(&self, column_index: usize) -> Result<String, DruidError> {
        self.call(
            "column_class_name",
            format!("com.example.Money{column_index}"),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
enum StrongGetterCall {
    BigDecimalIndex(usize, Option<i32>),
    BigDecimalLabel(String, Option<i32>),
    DateIndex(usize, RdbcCalendarArgument),
    DateLabel(String, RdbcCalendarArgument),
    TimeIndex(usize, RdbcCalendarArgument),
    TimeLabel(String, RdbcCalendarArgument),
    TimestampIndex(usize, RdbcCalendarArgument),
    TimestampLabel(String, RdbcCalendarArgument),
    RefIndex(usize),
    RefLabel(String),
    BlobIndex(usize),
    BlobLabel(String),
    ClobIndex(usize),
    ClobLabel(String),
    ArrayIndex(usize),
    ArrayLabel(String),
    UrlIndex(usize),
    UrlLabel(String),
    RowIdIndex(usize),
    RowIdLabel(String),
    NClobIndex(usize),
    NClobLabel(String),
    SqlXmlIndex(usize),
    SqlXmlLabel(String),
    UpdateRefIndex(usize, bool),
    UpdateRefLabel(String, bool),
    UpdateBlobIndex(usize, bool),
    UpdateBlobLabel(String, bool),
    UpdateClobIndex(usize, bool),
    UpdateClobLabel(String, bool),
    UpdateArrayIndex(usize, bool),
    UpdateArrayLabel(String, bool),
    UpdateRowIdIndex(usize, bool),
    UpdateRowIdLabel(String, bool),
    UpdateNClobIndex(usize, bool),
    UpdateNClobLabel(String, bool),
    UpdateSqlXmlIndex(usize, bool),
    UpdateSqlXmlLabel(String, bool),
    UpdateBlobStreamIndex(usize, bool, RdbcStreamLength),
    UpdateBlobStreamLabel(String, bool, RdbcStreamLength),
    UpdateClobReaderIndex(usize, bool, RdbcCharacterLength),
    UpdateClobReaderLabel(String, bool, RdbcCharacterLength),
    UpdateNClobReaderIndex(usize, bool, RdbcCharacterLength),
    UpdateNClobReaderLabel(String, bool, RdbcCharacterLength),
    UpdateValueIndex(usize, ResultSetUpdate),
    UpdateValueLabel(String, ResultSetUpdate),
    ObjectMapIndex(usize, Option<RdbcTypeMap>),
    ObjectMapLabel(String, Option<RdbcTypeMap>),
    ObjectTypedIndex(usize, RdbcTargetType),
    ObjectTypedLabel(String, RdbcTargetType),
}

#[derive(Debug)]
struct StrongGetterProbe {
    calls: Mutex<Vec<StrongGetterCall>>,
    closed: AtomicBool,
}

impl StrongGetterProbe {
    fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            closed: AtomicBool::new(false),
        }
    }

    fn calls(&self) -> Vec<StrongGetterCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl PhysicalResultSet for StrongGetterProbe {
    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn big_decimal(
        &self,
        column_index: usize,
        scale: Option<i32>,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::BigDecimalIndex(column_index, scale));
        Ok(Some(BigDecimal::from(1)))
    }

    fn big_decimal_by_label(
        &self,
        column_label: &str,
        scale: Option<i32>,
    ) -> Result<Option<BigDecimal>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::BigDecimalLabel(
                column_label.to_string(),
                scale,
            ));
        Ok(Some(BigDecimal::from(1)))
    }

    fn date(
        &self,
        column_index: usize,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::DateIndex(column_index, calendar.clone()));
        Ok(NaiveDate::from_ymd_opt(2026, 7, 29))
    }

    fn date_by_label(
        &self,
        column_label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.calls.lock().unwrap().push(StrongGetterCall::DateLabel(
            column_label.to_string(),
            calendar.clone(),
        ));
        Ok(NaiveDate::from_ymd_opt(2026, 7, 29))
    }

    fn time(
        &self,
        column_index: usize,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::TimeIndex(column_index, calendar.clone()));
        Ok(NaiveTime::from_hms_opt(13, 14, 15))
    }

    fn time_by_label(
        &self,
        column_label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.calls.lock().unwrap().push(StrongGetterCall::TimeLabel(
            column_label.to_string(),
            calendar.clone(),
        ));
        Ok(NaiveTime::from_hms_opt(13, 14, 15))
    }

    fn timestamp(
        &self,
        column_index: usize,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::TimestampIndex(
                column_index,
                calendar.clone(),
            ));
        Ok(NaiveDate::from_ymd_opt(2026, 7, 29).and_then(|date| date.and_hms_opt(13, 14, 15)))
    }

    fn timestamp_by_label(
        &self,
        column_label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::TimestampLabel(
                column_label.to_string(),
                calendar.clone(),
            ));
        Ok(NaiveDate::from_ymd_opt(2026, 7, 29).and_then(|date| date.and_hms_opt(13, 14, 15)))
    }

    fn reference(&self, column_index: usize) -> Result<Option<RdbcRef>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::RefIndex(column_index));
        Ok(None)
    }

    fn reference_by_label(&self, column_label: &str) -> Result<Option<RdbcRef>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::RefLabel(column_label.to_string()));
        Ok(None)
    }

    fn blob(&self, column_index: usize) -> Result<Option<RdbcBlob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::BlobIndex(column_index));
        Ok(None)
    }

    fn blob_by_label(&self, column_label: &str) -> Result<Option<RdbcBlob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::BlobLabel(column_label.to_string()));
        Ok(None)
    }

    fn clob(&self, column_index: usize) -> Result<Option<RdbcClob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ClobIndex(column_index));
        Ok(None)
    }

    fn clob_by_label(&self, column_label: &str) -> Result<Option<RdbcClob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ClobLabel(column_label.to_string()));
        Ok(None)
    }

    fn array(&self, column_index: usize) -> Result<Option<RdbcArray>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ArrayIndex(column_index));
        Ok(None)
    }

    fn array_by_label(&self, column_label: &str) -> Result<Option<RdbcArray>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ArrayLabel(column_label.to_string()));
        Ok(None)
    }

    fn url(&self, column_index: usize) -> Result<Option<RdbcUrl>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UrlIndex(column_index));
        Ok(None)
    }

    fn url_by_label(&self, column_label: &str) -> Result<Option<RdbcUrl>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UrlLabel(column_label.to_string()));
        Ok(None)
    }

    fn row_id(&self, column_index: usize) -> Result<Option<RdbcRowId>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::RowIdIndex(column_index));
        Ok(None)
    }

    fn row_id_by_label(&self, column_label: &str) -> Result<Option<RdbcRowId>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::RowIdLabel(column_label.to_string()));
        Ok(None)
    }

    fn n_clob(&self, column_index: usize) -> Result<Option<RdbcNClob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::NClobIndex(column_index));
        Ok(None)
    }

    fn n_clob_by_label(&self, column_label: &str) -> Result<Option<RdbcNClob>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::NClobLabel(column_label.to_string()));
        Ok(None)
    }

    fn sql_xml(&self, column_index: usize) -> Result<Option<RdbcSqlXml>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::SqlXmlIndex(column_index));
        Ok(None)
    }

    fn sql_xml_by_label(&self, column_label: &str) -> Result<Option<RdbcSqlXml>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::SqlXmlLabel(column_label.to_string()));
        Ok(None)
    }

    fn update_reference(
        &self,
        column_index: usize,
        value: Option<&RdbcRef>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateRefIndex(
                column_index,
                value.is_some(),
            ));
        Ok(())
    }

    fn update_reference_by_label(
        &self,
        column_label: &str,
        value: Option<&RdbcRef>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateRefLabel(
                column_label.to_string(),
                value.is_some(),
            ));
        Ok(())
    }

    fn update_blob(&self, column_index: usize, value: Option<&RdbcBlob>) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateBlobIndex(
                column_index,
                value.is_some(),
            ));
        Ok(())
    }

    fn update_blob_by_label(
        &self,
        column_label: &str,
        value: Option<&RdbcBlob>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateBlobLabel(
                column_label.to_string(),
                value.is_some(),
            ));
        Ok(())
    }

    fn update_clob(&self, column_index: usize, value: Option<&RdbcClob>) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateClobIndex(
                column_index,
                value.is_some(),
            ));
        Ok(())
    }

    fn update_clob_by_label(
        &self,
        column_label: &str,
        value: Option<&RdbcClob>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateClobLabel(
                column_label.to_string(),
                value.is_some(),
            ));
        Ok(())
    }

    fn update_array(
        &self,
        column_index: usize,
        value: Option<&RdbcArray>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateArrayIndex(
                column_index,
                value.is_some(),
            ));
        Ok(())
    }

    fn update_array_by_label(
        &self,
        column_label: &str,
        value: Option<&RdbcArray>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateArrayLabel(
                column_label.to_string(),
                value.is_some(),
            ));
        Ok(())
    }

    fn update_row_id(
        &self,
        column_index: usize,
        value: Option<&RdbcRowId>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateRowIdIndex(
                column_index,
                value.is_some(),
            ));
        Ok(())
    }

    fn update_row_id_by_label(
        &self,
        column_label: &str,
        value: Option<&RdbcRowId>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateRowIdLabel(
                column_label.to_string(),
                value.is_some(),
            ));
        Ok(())
    }

    fn update_n_clob(
        &self,
        column_index: usize,
        value: Option<&RdbcNClob>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateNClobIndex(
                column_index,
                value.is_some(),
            ));
        Ok(())
    }

    fn update_n_clob_by_label(
        &self,
        column_label: &str,
        value: Option<&RdbcNClob>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateNClobLabel(
                column_label.to_string(),
                value.is_some(),
            ));
        Ok(())
    }

    fn update_sql_xml(
        &self,
        column_index: usize,
        value: Option<&RdbcSqlXml>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateSqlXmlIndex(
                column_index,
                value.is_some(),
            ));
        Ok(())
    }

    fn update_sql_xml_by_label(
        &self,
        column_label: &str,
        value: Option<&RdbcSqlXml>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateSqlXmlLabel(
                column_label.to_string(),
                value.is_some(),
            ));
        Ok(())
    }

    fn update_blob_stream(
        &self,
        column_index: usize,
        stream: Option<&RdbcInputStream>,
        length: RdbcStreamLength,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateBlobStreamIndex(
                column_index,
                stream.is_some(),
                length,
            ));
        Ok(())
    }

    fn update_blob_stream_by_label(
        &self,
        column_label: &str,
        stream: Option<&RdbcInputStream>,
        length: RdbcStreamLength,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateBlobStreamLabel(
                column_label.to_string(),
                stream.is_some(),
                length,
            ));
        Ok(())
    }

    fn update_clob_reader(
        &self,
        column_index: usize,
        reader: Option<&RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateClobReaderIndex(
                column_index,
                reader.is_some(),
                length,
            ));
        Ok(())
    }

    fn update_clob_reader_by_label(
        &self,
        column_label: &str,
        reader: Option<&RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateClobReaderLabel(
                column_label.to_string(),
                reader.is_some(),
                length,
            ));
        Ok(())
    }

    fn update_n_clob_reader(
        &self,
        column_index: usize,
        reader: Option<&RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateNClobReaderIndex(
                column_index,
                reader.is_some(),
                length,
            ));
        Ok(())
    }

    fn update_n_clob_reader_by_label(
        &self,
        column_label: &str,
        reader: Option<&RdbcReader>,
        length: RdbcCharacterLength,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateNClobReaderLabel(
                column_label.to_string(),
                reader.is_some(),
                length,
            ));
        Ok(())
    }

    fn update_value(
        &self,
        column_index: usize,
        update: &ResultSetUpdate,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateValueIndex(
                column_index,
                update.clone(),
            ));
        Ok(())
    }

    fn update_value_by_label(
        &self,
        column_label: &str,
        update: &ResultSetUpdate,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::UpdateValueLabel(
                column_label.to_string(),
                update.clone(),
            ));
        Ok(())
    }

    fn object_with_type_map(
        &self,
        column_index: usize,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ObjectMapIndex(
                column_index,
                type_map.cloned(),
            ));
        Ok(RdbcObject::Scalar(Value::Int(71)))
    }

    fn object_by_label_with_type_map(
        &self,
        column_label: &str,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ObjectMapLabel(
                column_label.to_string(),
                type_map.cloned(),
            ));
        Ok(RdbcObject::Scalar(Value::Int(72)))
    }

    fn object_as(
        &self,
        column_index: usize,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ObjectTypedIndex(
                column_index,
                target_type.clone(),
            ));
        Ok(RdbcObject::String("typed-index".to_string()))
    }

    fn object_by_label_as(
        &self,
        column_label: &str,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ObjectTypedLabel(
                column_label.to_string(),
                target_type.clone(),
            ));
        Ok(RdbcObject::String("typed-label".to_string()))
    }
}

async fn sqlite_pooled_connection() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    DruidPooledConnection::new(physical, 17, Box::new(|_, _| {}))
}

#[tokio::test]
async fn sqlite_result_set_preserves_values_cursor_peak_and_wrapper_identity() {
    let mut connection = sqlite_pooled_connection().await;
    let mut statement = connection.create_statement().await.unwrap();
    statement
        .execute_update(
            &mut connection,
            "CREATE TABLE result_event (
                id INTEGER PRIMARY KEY,
                enabled INTEGER NOT NULL,
                score REAL NOT NULL,
                label TEXT NOT NULL,
                payload BLOB NOT NULL,
                optional_label TEXT
            )",
        )
        .await
        .unwrap();
    statement
        .execute_update(
            &mut connection,
            "INSERT INTO result_event VALUES
                (1, 1, 1.5, 'first', X'0102', NULL),
                (2, 0, 2.5, 'second', X'0304', 'present')",
        )
        .await
        .unwrap();

    let mut result_set = statement
        .execute_query_result_set(
            &mut connection,
            "SELECT id, enabled, score, label, payload, optional_label
             FROM result_event ORDER BY id",
        )
        .await
        .unwrap();

    assert_eq!(statement.fetch_row_peak(), -1);
    assert!(std::ptr::eq(
        statement.statement(),
        result_set.poolable_statement().statement()
    ));
    assert!(std::ptr::eq(
        statement.statement(),
        result_set.statement().statement()
    ));
    assert!(
        result_set.prepared_statement().is_none(),
        "普通 Statement 结果集不能伪装成 PreparedStatement"
    );
    assert!(
        result_set.callable_statement().is_none(),
        "普通 Statement 结果集不能伪装成 CallableStatement"
    );
    assert!(result_set.is_wrapper_for_type::<DruidPooledResultSet>());
    assert!(result_set.is_wrapper_for_type::<dyn PhysicalResultSet>());
    assert!(result_set.as_any().is::<DruidPooledResultSet>());
    assert!(result_set.unwrap_ref::<DruidPooledResultSet>().is_some());
    assert!(result_set
        .unwrap(Some(TypeId::of::<dyn PhysicalResultSet>()))
        .and_then(|value| value.result_set())
        .is_some());
    assert!(!result_set.is_wrapper_for(None));
    assert!(result_set.unwrap(None).is_none());
    assert!(format!("{result_set:?}").contains("DruidPooledResultSet"));

    assert!(result_set.is_before_first(&mut connection).unwrap());
    assert_eq!(result_set.row(&mut connection).unwrap(), 0);
    assert_eq!(result_set.warnings(&mut connection).unwrap(), None);
    result_set.clear_warnings(&mut connection).unwrap();
    assert_eq!(result_set.cursor_name(&mut connection).unwrap(), None);
    let meta_data = result_set.meta_data(&mut connection).unwrap();
    assert_eq!(meta_data.column_count().unwrap(), 6);
    assert_eq!(
        meta_data.column_type(1).unwrap(),
        ResultSetColumnType::Integer
    );
    assert_eq!(
        meta_data.column_type(3).unwrap(),
        ResultSetColumnType::Float
    );
    assert_eq!(meta_data.column_type(4).unwrap(), ResultSetColumnType::Text);
    assert_eq!(
        meta_data.column_type(5).unwrap(),
        ResultSetColumnType::Binary
    );
    assert!(meta_data.is_nullable(6).unwrap());
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(result_set.row(&mut connection).unwrap(), 1);
    assert_eq!(result_set.long(&mut connection, 1).unwrap(), 1);
    assert_eq!(result_set.int(&mut connection, 1).unwrap(), 1);
    assert_eq!(result_set.short(&mut connection, 1).unwrap(), 1);
    assert_eq!(result_set.byte(&mut connection, 1).unwrap(), 1);
    assert!(result_set.boolean(&mut connection, 2).unwrap());
    assert_eq!(result_set.double(&mut connection, 3).unwrap(), 1.5);
    assert_eq!(result_set.float(&mut connection, 3).unwrap(), 1.5);
    assert_eq!(
        result_set.string(&mut connection, 4).unwrap(),
        Some("first".to_string())
    );
    assert_eq!(
        result_set.bytes(&mut connection, 5).unwrap(),
        Some(vec![1, 2])
    );
    macro_rules! assert_sqlite_resource_unsupported {
        ($expression:expr, $operation:literal) => {
            assert!(matches!(
                $expression,
                Err(DruidError::UnsupportedOperation {
                    operation: $operation
                })
            ));
        };
    }
    assert_sqlite_resource_unsupported!(result_set.reference(&mut connection, 1), "result_set_ref");
    assert_sqlite_resource_unsupported!(
        result_set.reference_by_label(&mut connection, "id"),
        "result_set_ref_by_label"
    );
    assert_sqlite_resource_unsupported!(result_set.blob(&mut connection, 5), "result_set_blob");
    assert_sqlite_resource_unsupported!(
        result_set.blob_by_label(&mut connection, "payload"),
        "result_set_blob_by_label"
    );
    assert_sqlite_resource_unsupported!(result_set.clob(&mut connection, 4), "result_set_clob");
    assert_sqlite_resource_unsupported!(
        result_set.clob_by_label(&mut connection, "label"),
        "result_set_clob_by_label"
    );
    assert_sqlite_resource_unsupported!(result_set.array(&mut connection, 1), "result_set_array");
    assert_sqlite_resource_unsupported!(
        result_set.array_by_label(&mut connection, "id"),
        "result_set_array_by_label"
    );
    assert_sqlite_resource_unsupported!(result_set.url(&mut connection, 4), "result_set_url");
    assert_sqlite_resource_unsupported!(
        result_set.url_by_label(&mut connection, "label"),
        "result_set_url_by_label"
    );
    assert_sqlite_resource_unsupported!(result_set.row_id(&mut connection, 1), "result_set_row_id");
    assert_sqlite_resource_unsupported!(
        result_set.row_id_by_label(&mut connection, "id"),
        "result_set_row_id_by_label"
    );
    assert_sqlite_resource_unsupported!(result_set.n_clob(&mut connection, 4), "result_set_n_clob");
    assert_sqlite_resource_unsupported!(
        result_set.n_clob_by_label(&mut connection, "label"),
        "result_set_n_clob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.sql_xml(&mut connection, 4),
        "result_set_sql_xml"
    );
    assert_sqlite_resource_unsupported!(
        result_set.sql_xml_by_label(&mut connection, "label"),
        "result_set_sql_xml_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_reference(&mut connection, 1, None),
        "result_set_update_ref"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_reference_by_label(&mut connection, "id", None),
        "result_set_update_ref_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob(&mut connection, 5, None),
        "result_set_update_blob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_by_label(&mut connection, "payload", None),
        "result_set_update_blob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob(&mut connection, 4, None),
        "result_set_update_clob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_by_label(&mut connection, "label", None),
        "result_set_update_clob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_array(&mut connection, 1, None),
        "result_set_update_array"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_array_by_label(&mut connection, "id", None),
        "result_set_update_array_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_row_id(&mut connection, 1, None),
        "result_set_update_row_id"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_row_id_by_label(&mut connection, "id", None),
        "result_set_update_row_id_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob(&mut connection, 4, None),
        "result_set_update_n_clob"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_by_label(&mut connection, "label", None),
        "result_set_update_n_clob_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_sql_xml(&mut connection, 4, None),
        "result_set_update_sql_xml"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_sql_xml_by_label(&mut connection, "label", None),
        "result_set_update_sql_xml_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_stream(&mut connection, 5, None),
        "result_set_update_blob_stream"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_stream_by_label(&mut connection, "payload", None),
        "result_set_update_blob_stream_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_stream_with_length(&mut connection, 5, None, 1),
        "result_set_update_blob_stream"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_blob_stream_by_label_with_length(&mut connection, "payload", None, 1),
        "result_set_update_blob_stream_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_reader(&mut connection, 4, None),
        "result_set_update_clob_reader"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_reader_by_label(&mut connection, "label", None),
        "result_set_update_clob_reader_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_reader_with_length(&mut connection, 4, None, 1),
        "result_set_update_clob_reader"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_clob_reader_by_label_with_length(&mut connection, "label", None, 1),
        "result_set_update_clob_reader_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_reader(&mut connection, 4, None),
        "result_set_update_n_clob_reader"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_reader_by_label(&mut connection, "label", None),
        "result_set_update_n_clob_reader_by_label"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_reader_with_length(&mut connection, 4, None, 1),
        "result_set_update_n_clob_reader"
    );
    assert_sqlite_resource_unsupported!(
        result_set.update_n_clob_reader_by_label_with_length(&mut connection, "label", None, 1),
        "result_set_update_n_clob_reader_by_label"
    );
    assert_eq!(statement.exception_count(), 42);
    assert_eq!(
        result_set
            .binary_stream(&mut connection, 5)
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        vec![1, 2]
    );
    assert_eq!(
        result_set
            .ascii_stream(&mut connection, 4)
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"first"
    );
    assert_eq!(
        result_set
            .unicode_stream(&mut connection, 4)
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"first"
    );
    assert_eq!(
        result_set
            .character_stream(&mut connection, 4)
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "first"
    );
    assert_eq!(
        result_set
            .n_character_stream(&mut connection, 4)
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "first"
    );
    assert_eq!(
        result_set.n_string(&mut connection, 4).unwrap(),
        Some("first".to_string())
    );
    assert_eq!(result_set.object(&mut connection, 6).unwrap(), Value::Null);
    assert!(result_set.was_null(&mut connection).unwrap());
    assert!(result_set.is_first(&mut connection).unwrap());
    assert!(!result_set.row_updated(&mut connection).unwrap());
    assert!(!result_set.row_inserted(&mut connection).unwrap());
    assert!(!result_set.row_deleted(&mut connection).unwrap());

    for operation in [
        result_set.insert_row(&mut connection),
        result_set.update_row(&mut connection),
        result_set.delete_row(&mut connection),
        result_set.refresh_row(&mut connection),
        result_set.cancel_row_updates(&mut connection),
        result_set.move_to_insert_row(&mut connection),
        result_set.move_to_current_row(&mut connection),
    ] {
        assert!(matches!(
            operation,
            Err(DruidError::UnsupportedOperation { .. })
        ));
    }

    assert!(result_set.next(&mut connection).unwrap());
    assert!(result_set.is_last(&mut connection).unwrap());
    assert_eq!(result_set.fetch_row_count(), 2);
    assert!(result_set.previous(&mut connection).unwrap());
    assert_eq!(result_set.fetch_row_count(), 2);
    result_set.close_with_connection(&mut connection).unwrap();
    assert!(result_set.is_closed());
    assert_eq!(statement.fetch_row_peak(), 2);
}

#[tokio::test]
async fn sqlite_rejects_every_scalar_and_stream_update_without_corrupting_cursor() {
    let mut connection = sqlite_pooled_connection().await;
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(
            &mut connection,
            "SELECT 1 AS id, 'first' AS label UNION ALL SELECT 2, 'second'",
        )
        .await
        .unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    let input = RdbcInputStream::from_bytes([1, 2, 3]);
    let reader = RdbcReader::from_string("严格 SQLite");

    macro_rules! assert_update_unsupported {
        ($expression:expr, $operation:literal) => {
            assert!(matches!(
                $expression,
                Err(DruidError::UnsupportedOperation {
                    operation: $operation
                })
            ));
        };
    }
    macro_rules! assert_pair_unsupported {
        ($index_method:ident, $label_method:ident, $value:expr) => {
            assert_update_unsupported!(
                result_set.$index_method(&mut connection, 1, $value),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.$label_method(&mut connection, "id", $value),
                "result_set_update_value_by_label"
            );
        };
    }

    assert_update_unsupported!(
        result_set.update_null(&mut connection, 1),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_null_by_label(&mut connection, "id"),
        "result_set_update_value_by_label"
    );
    assert_pair_unsupported!(update_boolean, update_boolean_by_label, true);
    assert_pair_unsupported!(update_byte, update_byte_by_label, 1);
    assert_pair_unsupported!(update_short, update_short_by_label, 2);
    assert_pair_unsupported!(update_int, update_int_by_label, 3);
    assert_pair_unsupported!(update_long, update_long_by_label, 4);
    assert_pair_unsupported!(update_float, update_float_by_label, 5.0);
    assert_pair_unsupported!(update_double, update_double_by_label, 6.0);
    assert_pair_unsupported!(
        update_big_decimal,
        update_big_decimal_by_label,
        None::<BigDecimal>
    );
    assert_pair_unsupported!(update_string, update_string_by_label, None::<String>);
    assert_pair_unsupported!(update_bytes, update_bytes_by_label, None::<Vec<u8>>);
    assert_pair_unsupported!(update_date, update_date_by_label, None::<NaiveDate>);
    assert_pair_unsupported!(update_time, update_time_by_label, None::<NaiveTime>);
    assert_pair_unsupported!(
        update_timestamp,
        update_timestamp_by_label,
        None::<NaiveDateTime>
    );
    assert_update_unsupported!(
        result_set.update_object(&mut connection, 1, Value::Null.into()),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_object_by_label(&mut connection, "id", Value::Null.into()),
        "result_set_update_value_by_label"
    );
    assert_update_unsupported!(
        result_set.update_object_with_scale_or_length(&mut connection, 1, Value::Null.into(), -1),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_object_by_label_with_scale_or_length(
            &mut connection,
            "id",
            Value::Null.into(),
            -1,
        ),
        "result_set_update_value_by_label"
    );
    assert_pair_unsupported!(update_n_string, update_n_string_by_label, None::<String>);

    macro_rules! assert_stream_family_unsupported {
        (
            $plain_index:ident, $plain_label:ident,
            $int_index:ident, $int_label:ident,
            $long_index:ident, $long_label:ident
        ) => {
            assert_update_unsupported!(
                result_set.$plain_index(&mut connection, 1, Some(&input)),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.$plain_label(&mut connection, "id", Some(&input)),
                "result_set_update_value_by_label"
            );
            assert_update_unsupported!(
                result_set.$int_index(&mut connection, 1, Some(&input), -1),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.$int_label(&mut connection, "id", Some(&input), -1),
                "result_set_update_value_by_label"
            );
            assert_update_unsupported!(
                result_set.$long_index(&mut connection, 1, Some(&input), -1),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.$long_label(&mut connection, "id", Some(&input), -1),
                "result_set_update_value_by_label"
            );
        };
    }
    assert_stream_family_unsupported!(
        update_ascii_stream,
        update_ascii_stream_by_label,
        update_ascii_stream_with_int_length,
        update_ascii_stream_by_label_with_int_length,
        update_ascii_stream_with_length,
        update_ascii_stream_by_label_with_length
    );
    assert_stream_family_unsupported!(
        update_binary_stream,
        update_binary_stream_by_label,
        update_binary_stream_with_int_length,
        update_binary_stream_by_label_with_int_length,
        update_binary_stream_with_length,
        update_binary_stream_by_label_with_length
    );

    macro_rules! assert_reader_family_unsupported {
        (
            $plain_index:ident, $plain_label:ident,
            $int_index:ident, $int_label:ident,
            $long_index:ident, $long_label:ident
        ) => {
            assert_update_unsupported!(
                result_set.$plain_index(&mut connection, 1, Some(&reader)),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.$plain_label(&mut connection, "id", Some(&reader)),
                "result_set_update_value_by_label"
            );
            assert_update_unsupported!(
                result_set.$int_index(&mut connection, 1, Some(&reader), -1),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.$int_label(&mut connection, "id", Some(&reader), -1),
                "result_set_update_value_by_label"
            );
            assert_update_unsupported!(
                result_set.$long_index(&mut connection, 1, Some(&reader), -1),
                "result_set_update_value"
            );
            assert_update_unsupported!(
                result_set.$long_label(&mut connection, "id", Some(&reader), -1),
                "result_set_update_value_by_label"
            );
        };
    }
    assert_reader_family_unsupported!(
        update_character_stream,
        update_character_stream_by_label,
        update_character_stream_with_int_length,
        update_character_stream_by_label_with_int_length,
        update_character_stream_with_length,
        update_character_stream_by_label_with_length
    );
    assert_update_unsupported!(
        result_set.update_n_character_stream(&mut connection, 1, Some(&reader)),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_n_character_stream_by_label(&mut connection, "id", Some(&reader)),
        "result_set_update_value_by_label"
    );
    assert_update_unsupported!(
        result_set.update_n_character_stream_with_length(&mut connection, 1, Some(&reader), -1),
        "result_set_update_value"
    );
    assert_update_unsupported!(
        result_set.update_n_character_stream_by_label_with_length(
            &mut connection,
            "id",
            Some(&reader),
            -1,
        ),
        "result_set_update_value_by_label"
    );

    let mut type_map = RdbcTypeMap::new();
    type_map.insert("APP.USER_TYPE", RdbcTargetType::Custom("User".to_string()));
    assert_update_unsupported!(
        result_set.object_with_type_map(&mut connection, 1, Some(&type_map)),
        "result_set_get_object_with_type_map"
    );
    assert_update_unsupported!(
        result_set.object_by_label_with_type_map(&mut connection, "id", None),
        "result_set_get_object_by_label_with_type_map"
    );

    assert_eq!(statement.exception_count(), 58);
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(result_set.int(&mut connection, 1).unwrap(), 2);
    assert_eq!(
        result_set.string(&mut connection, 2).unwrap(),
        Some("second".to_string())
    );
    assert!(!input.is_closed());
    assert!(!reader.is_closed());
}

#[tokio::test]
async fn map_get_object_overloads_preserve_index_label_and_nullable_map_identity() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let probe = Arc::new(StrongGetterProbe::new());
    let physical: Arc<dyn PhysicalResultSet> = probe.clone();
    let mut result_set = statement.wrap_result_set(physical).unwrap();
    let mut type_map = RdbcTypeMap::new();
    type_map.insert("APP.MONEY", RdbcTargetType::BigDecimal);

    assert_eq!(
        result_set
            .object_with_type_map(&mut connection, 31, Some(&type_map))
            .unwrap(),
        RdbcObject::Scalar(Value::Int(71))
    );
    assert_eq!(
        result_set
            .object_by_label_with_type_map(&mut connection, "mapped", None)
            .unwrap(),
        RdbcObject::Scalar(Value::Int(72))
    );
    assert_eq!(
        probe.calls(),
        vec![
            StrongGetterCall::ObjectMapIndex(31, Some(type_map)),
            StrongGetterCall::ObjectMapLabel("mapped".to_string(), None),
        ]
    );
}

#[tokio::test]
async fn typed_get_object_overloads_preserve_raw_target_type_identity() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let probe = Arc::new(StrongGetterProbe::new());
    let physical: Arc<dyn PhysicalResultSet> = probe.clone();
    let mut result_set = statement.wrap_result_set(physical).unwrap();

    assert_eq!(
        result_set
            .object_typed(
                &mut connection,
                41,
                &RdbcTargetType::Custom("com.example.Money".to_string()),
            )
            .unwrap(),
        RdbcObject::String("typed-index".to_string())
    );
    assert_eq!(
        result_set
            .object_typed_by_label(&mut connection, "payload", &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::String("typed-label".to_string())
    );
    assert_eq!(
        probe.calls(),
        vec![
            StrongGetterCall::ObjectTypedIndex(
                41,
                RdbcTargetType::Custom("com.example.Money".to_string())
            ),
            StrongGetterCall::ObjectTypedLabel("payload".to_string(), RdbcTargetType::Bytes),
        ]
    );
}

#[test]
fn default_typed_get_object_delegates_every_standard_resource_target() {
    let probe = TypedResourceProbe::new();
    let null = RdbcObject::Scalar(Value::Null);

    assert_eq!(probe.object_as(1, &RdbcTargetType::Blob).unwrap(), null);
    assert_eq!(probe.object_as(2, &RdbcTargetType::Clob).unwrap(), null);
    assert_eq!(probe.object_as(3, &RdbcTargetType::NClob).unwrap(), null);
    assert_eq!(probe.object_as(4, &RdbcTargetType::Array).unwrap(), null);
    assert_eq!(probe.object_as(5, &RdbcTargetType::Ref).unwrap(), null);
    assert_eq!(probe.object_as(6, &RdbcTargetType::RowId).unwrap(), null);
    assert_eq!(probe.object_as(7, &RdbcTargetType::SqlXml).unwrap(), null);
    assert_eq!(probe.object_as(8, &RdbcTargetType::Url).unwrap(), null);
    assert_eq!(
        probe
            .object_by_label_as("blob_label", &RdbcTargetType::Blob)
            .unwrap(),
        null
    );
    assert_eq!(
        probe.calls(),
        vec![
            TypedResourceCall::Blob(1),
            TypedResourceCall::Clob(2),
            TypedResourceCall::NClob(3),
            TypedResourceCall::Array(4),
            TypedResourceCall::Ref(5),
            TypedResourceCall::RowId(6),
            TypedResourceCall::SqlXml(7),
            TypedResourceCall::Url(8),
            TypedResourceCall::FindColumn("blob_label".to_string()),
            TypedResourceCall::Blob(41),
        ]
    );
}

#[test]
fn rdbc_object_and_opaque_object_preserve_type_identity_and_display_contract() {
    let physical = Arc::new(VendorObjectProbe { id: 99 });
    let opaque = RdbcOpaqueObject::new(physical.clone());
    let same = opaque.clone();
    let other = RdbcOpaqueObject::new(Arc::new(VendorObjectProbe { id: 99 }));

    assert_eq!(opaque.class_name(), "com.example.VendorObject");
    assert_eq!(
        opaque.downcast_ref::<VendorObjectProbe>(),
        Some(&VendorObjectProbe { id: 99 })
    );
    assert!(opaque.downcast_ref::<String>().is_none());
    assert_eq!(opaque.physical().class_name(), "com.example.VendorObject");
    assert!(format!("{opaque:?}").contains("com.example.VendorObject"));
    assert_eq!(opaque, same);
    assert_ne!(opaque, other);

    let values = [
        (RdbcObject::String("text".to_string()), "text"),
        (RdbcObject::Boolean(true), "true"),
        (RdbcObject::Byte(-1), "-1"),
        (RdbcObject::Short(-2), "-2"),
        (RdbcObject::Integer(-3), "-3"),
        (RdbcObject::Long(-4), "-4"),
        (RdbcObject::Float(1.25), "1.25"),
        (RdbcObject::Double(2.5), "2.5"),
        (RdbcObject::Bytes(vec![0, 1, 2]), "<3 bytes>"),
        (RdbcObject::Custom(opaque), "<com.example.VendorObject>"),
    ];
    for (value, expected) in values {
        assert_eq!(value.to_string(), expected);
        assert!(!value.is_null());
    }
    assert!(RdbcObject::from(Value::Null).is_null());
}

#[tokio::test]
async fn sqlite_typed_get_object_converts_all_standard_scalar_targets() {
    let mut connection = sqlite_pooled_connection().await;
    let mut statement = connection.create_statement().await.unwrap();
    statement
        .execute_update(
            &mut connection,
            "CREATE TABLE typed_value (
                bool_text TEXT,
                byte_value INTEGER,
                short_value INTEGER,
                int_value INTEGER,
                long_value INTEGER,
                float_value REAL,
                double_value REAL,
                decimal_value DECIMAL,
                date_value DATE,
                time_value TIME,
                timestamp_value TIMESTAMP,
                text_value TEXT,
                bytes_value BLOB,
                null_value TEXT,
                overflow_byte INTEGER
            )",
        )
        .await
        .unwrap();
    statement
        .execute_update(
            &mut connection,
            "INSERT INTO typed_value VALUES (
                '1', 127, 32000, 2147483647, 9223372036854775807,
                1.25, 2.5, '1234567890.123456789',
                '2026-07-29', '13:14:15.123456789',
                '2026-07-29 13:14:15.123456789',
                'druid', X'00FF', NULL, 256
            )",
        )
        .await
        .unwrap();
    let mut result_set = statement
        .execute_query_result_set(
            &mut connection,
            "SELECT bool_text, byte_value, short_value, int_value, long_value,
                    float_value, double_value, decimal_value, date_value, time_value,
                    timestamp_value, text_value, bytes_value, null_value, overflow_byte
             FROM typed_value",
        )
        .await
        .unwrap();
    assert!(result_set.next(&mut connection).unwrap());

    assert_eq!(
        result_set
            .object_typed(&mut connection, 1, &RdbcTargetType::Boolean)
            .unwrap(),
        RdbcObject::Boolean(true)
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 2, &RdbcTargetType::Byte)
            .unwrap(),
        RdbcObject::Byte(127)
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 3, &RdbcTargetType::Short)
            .unwrap(),
        RdbcObject::Short(32_000)
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 4, &RdbcTargetType::Integer)
            .unwrap(),
        RdbcObject::Integer(i32::MAX)
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 5, &RdbcTargetType::Long)
            .unwrap(),
        RdbcObject::Long(i64::MAX)
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 6, &RdbcTargetType::Float)
            .unwrap(),
        RdbcObject::Float(1.25)
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 7, &RdbcTargetType::Double)
            .unwrap(),
        RdbcObject::Double(2.5)
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 8, &RdbcTargetType::BigDecimal)
            .unwrap(),
        // SQLite NUMERIC affinity stores this literal as IEEE-754 and therefore
        // exposes the same rounded value a RDBC SQLite driver would return.
        RdbcObject::BigDecimal(BigDecimal::from_str("1234567890.1234567").unwrap())
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 9, &RdbcTargetType::Date)
            .unwrap(),
        RdbcObject::Date(NaiveDate::from_ymd_opt(2026, 7, 29).unwrap())
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 10, &RdbcTargetType::Time)
            .unwrap(),
        RdbcObject::Time(NaiveTime::from_hms_nano_opt(13, 14, 15, 123_456_789).unwrap())
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 11, &RdbcTargetType::Timestamp)
            .unwrap(),
        RdbcObject::Timestamp(
            NaiveDate::from_ymd_opt(2026, 7, 29)
                .unwrap()
                .and_hms_nano_opt(13, 14, 15, 123_456_789)
                .unwrap()
        )
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 4, &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::String(i32::MAX.to_string())
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 12, &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::Bytes(b"druid".to_vec())
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 13, &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::Bytes(vec![0, 255])
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 14, &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::Scalar(Value::Null)
    );
    assert!(result_set.was_null(&mut connection).unwrap());

    assert!(matches!(
        result_set.object_typed(&mut connection, 15, &RdbcTargetType::Byte),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.object_typed(
            &mut connection,
            12,
            &RdbcTargetType::Custom("vendor.Type".to_string()),
        ),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_get_object_typed_custom"
        })
    ));
    assert_eq!(statement.exception_count(), 2);
}

#[tokio::test]
async fn all_strong_getter_overloads_preserve_raw_spi_argument_identity() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let probe = Arc::new(StrongGetterProbe::new());
    let physical: Arc<dyn PhysicalResultSet> = probe.clone();
    let mut result_set = statement.wrap_result_set(physical).unwrap();
    let shanghai = RdbcCalendar::new("Asia/Shanghai").unwrap();

    result_set.big_decimal(&mut connection, 1).unwrap();
    result_set
        .big_decimal_by_label(&mut connection, "amount")
        .unwrap();
    result_set
        .big_decimal_with_scale(&mut connection, 2, 7)
        .unwrap();
    result_set
        .big_decimal_by_label_with_scale(&mut connection, "scaled_amount", 8)
        .unwrap();

    result_set.date(&mut connection, 3).unwrap();
    result_set
        .date_by_label(&mut connection, "event_date")
        .unwrap();
    result_set
        .date_with_calendar(&mut connection, 4, Some(shanghai.clone()))
        .unwrap();
    result_set
        .date_by_label_with_calendar(&mut connection, "nullable_date", None)
        .unwrap();

    result_set.time(&mut connection, 5).unwrap();
    result_set
        .time_by_label(&mut connection, "event_time")
        .unwrap();
    result_set
        .time_with_calendar(&mut connection, 6, None)
        .unwrap();
    result_set
        .time_by_label_with_calendar(&mut connection, "zoned_time", Some(shanghai.clone()))
        .unwrap();

    result_set.timestamp(&mut connection, 7).unwrap();
    result_set
        .timestamp_by_label(&mut connection, "event_at")
        .unwrap();
    result_set
        .timestamp_with_calendar(&mut connection, 8, Some(shanghai.clone()))
        .unwrap();
    result_set
        .timestamp_by_label_with_calendar(&mut connection, "nullable_at", None)
        .unwrap();

    assert_eq!(result_set.reference(&mut connection, 9).unwrap(), None);
    assert_eq!(
        result_set
            .reference_by_label(&mut connection, "reference")
            .unwrap(),
        None
    );
    assert_eq!(result_set.blob(&mut connection, 10).unwrap(), None);
    assert_eq!(
        result_set
            .blob_by_label(&mut connection, "binary_lob")
            .unwrap(),
        None
    );
    assert_eq!(result_set.clob(&mut connection, 11).unwrap(), None);
    assert_eq!(
        result_set
            .clob_by_label(&mut connection, "character_lob")
            .unwrap(),
        None
    );
    assert_eq!(result_set.array(&mut connection, 12).unwrap(), None);
    assert_eq!(
        result_set
            .array_by_label(&mut connection, "array_value")
            .unwrap(),
        None
    );
    assert_eq!(result_set.url(&mut connection, 13).unwrap(), None);
    assert_eq!(
        result_set
            .url_by_label(&mut connection, "url_value")
            .unwrap(),
        None
    );
    assert_eq!(result_set.row_id(&mut connection, 14).unwrap(), None);
    assert_eq!(
        result_set
            .row_id_by_label(&mut connection, "row_identifier")
            .unwrap(),
        None
    );
    assert_eq!(result_set.n_clob(&mut connection, 15).unwrap(), None);
    assert_eq!(
        result_set
            .n_clob_by_label(&mut connection, "national_lob")
            .unwrap(),
        None
    );
    assert_eq!(result_set.sql_xml(&mut connection, 16).unwrap(), None);
    assert_eq!(
        result_set
            .sql_xml_by_label(&mut connection, "xml_value")
            .unwrap(),
        None
    );
    result_set
        .update_reference(&mut connection, 17, None)
        .unwrap();
    result_set
        .update_reference_by_label(&mut connection, "reference_update", None)
        .unwrap();
    result_set.update_blob(&mut connection, 18, None).unwrap();
    result_set
        .update_blob_by_label(&mut connection, "blob_update", None)
        .unwrap();
    result_set.update_clob(&mut connection, 19, None).unwrap();
    result_set
        .update_clob_by_label(&mut connection, "clob_update", None)
        .unwrap();
    result_set.update_array(&mut connection, 20, None).unwrap();
    result_set
        .update_array_by_label(&mut connection, "array_update", None)
        .unwrap();
    result_set.update_row_id(&mut connection, 21, None).unwrap();
    result_set
        .update_row_id_by_label(&mut connection, "row_id_update", None)
        .unwrap();
    result_set.update_n_clob(&mut connection, 22, None).unwrap();
    result_set
        .update_n_clob_by_label(&mut connection, "n_clob_update", None)
        .unwrap();
    result_set
        .update_sql_xml(&mut connection, 23, None)
        .unwrap();
    result_set
        .update_sql_xml_by_label(&mut connection, "sql_xml_update", None)
        .unwrap();
    result_set
        .update_blob_stream(&mut connection, 24, None)
        .unwrap();
    result_set
        .update_blob_stream_by_label(&mut connection, "blob_stream", None)
        .unwrap();
    result_set
        .update_blob_stream_with_length(&mut connection, 25, None, 101)
        .unwrap();
    result_set
        .update_blob_stream_by_label_with_length(&mut connection, "blob_stream_length", None, 102)
        .unwrap();
    result_set
        .update_clob_reader(&mut connection, 26, None)
        .unwrap();
    result_set
        .update_clob_reader_by_label(&mut connection, "clob_reader", None)
        .unwrap();
    result_set
        .update_clob_reader_with_length(&mut connection, 27, None, 103)
        .unwrap();
    result_set
        .update_clob_reader_by_label_with_length(&mut connection, "clob_reader_length", None, 104)
        .unwrap();
    result_set
        .update_n_clob_reader(&mut connection, 28, None)
        .unwrap();
    result_set
        .update_n_clob_reader_by_label(&mut connection, "n_clob_reader", None)
        .unwrap();
    result_set
        .update_n_clob_reader_with_length(&mut connection, 29, None, 105)
        .unwrap();
    result_set
        .update_n_clob_reader_by_label_with_length(
            &mut connection,
            "n_clob_reader_length",
            None,
            106,
        )
        .unwrap();

    assert_eq!(
        probe.calls(),
        vec![
            StrongGetterCall::BigDecimalIndex(1, None),
            StrongGetterCall::BigDecimalLabel("amount".to_string(), None),
            StrongGetterCall::BigDecimalIndex(2, Some(7)),
            StrongGetterCall::BigDecimalLabel("scaled_amount".to_string(), Some(8)),
            StrongGetterCall::DateIndex(3, RdbcCalendarArgument::Unspecified),
            StrongGetterCall::DateLabel(
                "event_date".to_string(),
                RdbcCalendarArgument::Unspecified
            ),
            StrongGetterCall::DateIndex(4, RdbcCalendarArgument::Specified(Some(shanghai.clone()))),
            StrongGetterCall::DateLabel(
                "nullable_date".to_string(),
                RdbcCalendarArgument::Specified(None)
            ),
            StrongGetterCall::TimeIndex(5, RdbcCalendarArgument::Unspecified),
            StrongGetterCall::TimeLabel(
                "event_time".to_string(),
                RdbcCalendarArgument::Unspecified
            ),
            StrongGetterCall::TimeIndex(6, RdbcCalendarArgument::Specified(None)),
            StrongGetterCall::TimeLabel(
                "zoned_time".to_string(),
                RdbcCalendarArgument::Specified(Some(shanghai.clone()))
            ),
            StrongGetterCall::TimestampIndex(7, RdbcCalendarArgument::Unspecified),
            StrongGetterCall::TimestampLabel(
                "event_at".to_string(),
                RdbcCalendarArgument::Unspecified
            ),
            StrongGetterCall::TimestampIndex(8, RdbcCalendarArgument::Specified(Some(shanghai))),
            StrongGetterCall::TimestampLabel(
                "nullable_at".to_string(),
                RdbcCalendarArgument::Specified(None)
            ),
            StrongGetterCall::RefIndex(9),
            StrongGetterCall::RefLabel("reference".to_string()),
            StrongGetterCall::BlobIndex(10),
            StrongGetterCall::BlobLabel("binary_lob".to_string()),
            StrongGetterCall::ClobIndex(11),
            StrongGetterCall::ClobLabel("character_lob".to_string()),
            StrongGetterCall::ArrayIndex(12),
            StrongGetterCall::ArrayLabel("array_value".to_string()),
            StrongGetterCall::UrlIndex(13),
            StrongGetterCall::UrlLabel("url_value".to_string()),
            StrongGetterCall::RowIdIndex(14),
            StrongGetterCall::RowIdLabel("row_identifier".to_string()),
            StrongGetterCall::NClobIndex(15),
            StrongGetterCall::NClobLabel("national_lob".to_string()),
            StrongGetterCall::SqlXmlIndex(16),
            StrongGetterCall::SqlXmlLabel("xml_value".to_string()),
            StrongGetterCall::UpdateRefIndex(17, false),
            StrongGetterCall::UpdateRefLabel("reference_update".to_string(), false),
            StrongGetterCall::UpdateBlobIndex(18, false),
            StrongGetterCall::UpdateBlobLabel("blob_update".to_string(), false),
            StrongGetterCall::UpdateClobIndex(19, false),
            StrongGetterCall::UpdateClobLabel("clob_update".to_string(), false),
            StrongGetterCall::UpdateArrayIndex(20, false),
            StrongGetterCall::UpdateArrayLabel("array_update".to_string(), false),
            StrongGetterCall::UpdateRowIdIndex(21, false),
            StrongGetterCall::UpdateRowIdLabel("row_id_update".to_string(), false),
            StrongGetterCall::UpdateNClobIndex(22, false),
            StrongGetterCall::UpdateNClobLabel("n_clob_update".to_string(), false),
            StrongGetterCall::UpdateSqlXmlIndex(23, false),
            StrongGetterCall::UpdateSqlXmlLabel("sql_xml_update".to_string(), false),
            StrongGetterCall::UpdateBlobStreamIndex(24, false, RdbcStreamLength::Unspecified),
            StrongGetterCall::UpdateBlobStreamLabel(
                "blob_stream".to_string(),
                false,
                RdbcStreamLength::Unspecified
            ),
            StrongGetterCall::UpdateBlobStreamIndex(25, false, RdbcStreamLength::Long(101)),
            StrongGetterCall::UpdateBlobStreamLabel(
                "blob_stream_length".to_string(),
                false,
                RdbcStreamLength::Long(102)
            ),
            StrongGetterCall::UpdateClobReaderIndex(26, false, RdbcCharacterLength::Unspecified),
            StrongGetterCall::UpdateClobReaderLabel(
                "clob_reader".to_string(),
                false,
                RdbcCharacterLength::Unspecified
            ),
            StrongGetterCall::UpdateClobReaderIndex(27, false, RdbcCharacterLength::Long(103)),
            StrongGetterCall::UpdateClobReaderLabel(
                "clob_reader_length".to_string(),
                false,
                RdbcCharacterLength::Long(104)
            ),
            StrongGetterCall::UpdateNClobReaderIndex(28, false, RdbcCharacterLength::Unspecified),
            StrongGetterCall::UpdateNClobReaderLabel(
                "n_clob_reader".to_string(),
                false,
                RdbcCharacterLength::Unspecified
            ),
            StrongGetterCall::UpdateNClobReaderIndex(29, false, RdbcCharacterLength::Long(105)),
            StrongGetterCall::UpdateNClobReaderLabel(
                "n_clob_reader_length".to_string(),
                false,
                RdbcCharacterLength::Long(106)
            ),
        ]
    );
}

#[tokio::test]
async fn all_scalar_and_stream_update_overloads_preserve_raw_spi_argument_identity() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let probe = Arc::new(StrongGetterProbe::new());
    let physical: Arc<dyn PhysicalResultSet> = probe.clone();
    let mut result_set = statement.wrap_result_set(physical).unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(13, 14, 15, 123_456_789).unwrap();
    let timestamp = date.and_time(time);
    let input = RdbcInputStream::from_bytes([1, 2, 3]);
    let reader = RdbcReader::from_string("严格流身份");
    let vendor_object = RdbcOpaqueObject::new(Arc::new(VendorObjectProbe { id: 99 }));
    assert_eq!(
        vendor_object.downcast_ref::<VendorObjectProbe>(),
        Some(&VendorObjectProbe { id: 99 })
    );

    macro_rules! assert_last_update {
        ($expression:expr, $expected:expr) => {{
            $expression.unwrap();
            assert_eq!(probe.calls().last().cloned(), Some($expected));
        }};
    }

    assert_last_update!(
        result_set.update_null(&mut connection, 1),
        StrongGetterCall::UpdateValueIndex(1, ResultSetUpdate::Null)
    );
    assert_last_update!(
        result_set.update_null_by_label(&mut connection, "null_value"),
        StrongGetterCall::UpdateValueLabel("null_value".to_string(), ResultSetUpdate::Null)
    );

    macro_rules! assert_scalar_pair {
        (
            $index_method:ident, $label_method:ident, $index:expr, $label:literal,
            $index_value:expr, $label_value:expr, $index_update:expr, $label_update:expr
        ) => {
            assert_last_update!(
                result_set.$index_method(&mut connection, $index, $index_value),
                StrongGetterCall::UpdateValueIndex($index, $index_update)
            );
            assert_last_update!(
                result_set.$label_method(&mut connection, $label, $label_value),
                StrongGetterCall::UpdateValueLabel($label.to_string(), $label_update)
            );
        };
    }

    assert_scalar_pair!(
        update_boolean,
        update_boolean_by_label,
        2,
        "boolean_value",
        true,
        false,
        ResultSetUpdate::Boolean(true),
        ResultSetUpdate::Boolean(false)
    );
    assert_scalar_pair!(
        update_byte,
        update_byte_by_label,
        3,
        "byte_value",
        -8,
        8,
        ResultSetUpdate::Byte(-8),
        ResultSetUpdate::Byte(8)
    );
    assert_scalar_pair!(
        update_short,
        update_short_by_label,
        4,
        "short_value",
        -16,
        16,
        ResultSetUpdate::Short(-16),
        ResultSetUpdate::Short(16)
    );
    assert_scalar_pair!(
        update_int,
        update_int_by_label,
        5,
        "int_value",
        -32,
        32,
        ResultSetUpdate::Int(-32),
        ResultSetUpdate::Int(32)
    );
    assert_scalar_pair!(
        update_long,
        update_long_by_label,
        6,
        "long_value",
        -64,
        64,
        ResultSetUpdate::Long(-64),
        ResultSetUpdate::Long(64)
    );
    assert_scalar_pair!(
        update_float,
        update_float_by_label,
        7,
        "float_value",
        1.25,
        -1.25,
        ResultSetUpdate::Float(1.25),
        ResultSetUpdate::Float(-1.25)
    );
    assert_scalar_pair!(
        update_double,
        update_double_by_label,
        8,
        "double_value",
        2.5,
        -2.5,
        ResultSetUpdate::Double(2.5),
        ResultSetUpdate::Double(-2.5)
    );
    let decimal = BigDecimal::from_str("1234567890.123456789").unwrap();
    assert_scalar_pair!(
        update_big_decimal,
        update_big_decimal_by_label,
        9,
        "decimal_value",
        Some(decimal.clone()),
        None,
        ResultSetUpdate::BigDecimal(Some(decimal)),
        ResultSetUpdate::BigDecimal(None)
    );
    assert_scalar_pair!(
        update_string,
        update_string_by_label,
        10,
        "string_value",
        Some("index".to_string()),
        None,
        ResultSetUpdate::String(Some("index".to_string())),
        ResultSetUpdate::String(None)
    );
    assert_scalar_pair!(
        update_bytes,
        update_bytes_by_label,
        11,
        "bytes_value",
        Some(vec![0, 255]),
        None,
        ResultSetUpdate::Bytes(Some(vec![0, 255])),
        ResultSetUpdate::Bytes(None)
    );
    assert_scalar_pair!(
        update_date,
        update_date_by_label,
        12,
        "date_value",
        Some(date),
        None,
        ResultSetUpdate::Date(Some(date)),
        ResultSetUpdate::Date(None)
    );
    assert_scalar_pair!(
        update_time,
        update_time_by_label,
        13,
        "time_value",
        Some(time),
        None,
        ResultSetUpdate::Time(Some(time)),
        ResultSetUpdate::Time(None)
    );
    assert_scalar_pair!(
        update_timestamp,
        update_timestamp_by_label,
        14,
        "timestamp_value",
        Some(timestamp),
        None,
        ResultSetUpdate::Timestamp(Some(timestamp)),
        ResultSetUpdate::Timestamp(None)
    );
    assert_last_update!(
        result_set.update_object(
            &mut connection,
            15,
            RdbcObject::Custom(vendor_object.clone())
        ),
        StrongGetterCall::UpdateValueIndex(
            15,
            ResultSetUpdate::Object(RdbcObject::Custom(vendor_object))
        )
    );
    assert_last_update!(
        result_set.update_object_by_label(&mut connection, "object_value", Value::Null.into()),
        StrongGetterCall::UpdateValueLabel(
            "object_value".to_string(),
            ResultSetUpdate::Object(Value::Null.into())
        )
    );
    assert_last_update!(
        result_set.update_object_with_scale_or_length(
            &mut connection,
            16,
            Value::Decimal(BigDecimal::from(7)).into(),
            -3,
        ),
        StrongGetterCall::UpdateValueIndex(
            16,
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Decimal(BigDecimal::from(7)).into(),
                scale_or_length: -3,
            }
        )
    );
    assert_last_update!(
        result_set.update_object_by_label_with_scale_or_length(
            &mut connection,
            "scaled_object",
            Value::Bytes(vec![9]).into(),
            99,
        ),
        StrongGetterCall::UpdateValueLabel(
            "scaled_object".to_string(),
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Bytes(vec![9]).into(),
                scale_or_length: 99,
            }
        )
    );
    assert_scalar_pair!(
        update_n_string,
        update_n_string_by_label,
        17,
        "n_string_value",
        Some("国家".to_string()),
        None,
        ResultSetUpdate::NString(Some("国家".to_string())),
        ResultSetUpdate::NString(None)
    );

    macro_rules! assert_stream_call {
        ($expression:expr, $target:expr, $variant:ident, $length:expr) => {
            assert_last_update!(
                $expression,
                $target(ResultSetUpdate::$variant {
                    stream: Some(input.clone()),
                    length: $length,
                })
            );
        };
    }
    assert_stream_call!(
        result_set.update_ascii_stream(&mut connection, 18, Some(&input)),
        |update| StrongGetterCall::UpdateValueIndex(18, update),
        AsciiStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_ascii_stream_by_label(&mut connection, "ascii", Some(&input)),
        |update| StrongGetterCall::UpdateValueLabel("ascii".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_ascii_stream_with_int_length(&mut connection, 19, Some(&input), -19),
        |update| StrongGetterCall::UpdateValueIndex(19, update),
        AsciiStream,
        RdbcStreamLength::Int(-19)
    );
    assert_stream_call!(
        result_set.update_ascii_stream_by_label_with_int_length(
            &mut connection,
            "ascii_int",
            Some(&input),
            20,
        ),
        |update| StrongGetterCall::UpdateValueLabel("ascii_int".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Int(20)
    );
    assert_stream_call!(
        result_set.update_ascii_stream_with_length(&mut connection, 20, Some(&input), -21),
        |update| StrongGetterCall::UpdateValueIndex(20, update),
        AsciiStream,
        RdbcStreamLength::Long(-21)
    );
    assert_stream_call!(
        result_set.update_ascii_stream_by_label_with_length(
            &mut connection,
            "ascii_long",
            Some(&input),
            22,
        ),
        |update| StrongGetterCall::UpdateValueLabel("ascii_long".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Long(22)
    );
    assert_stream_call!(
        result_set.update_binary_stream(&mut connection, 21, Some(&input)),
        |update| StrongGetterCall::UpdateValueIndex(21, update),
        BinaryStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_binary_stream_by_label(&mut connection, "binary", Some(&input)),
        |update| StrongGetterCall::UpdateValueLabel("binary".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_binary_stream_with_int_length(&mut connection, 22, Some(&input), -23),
        |update| StrongGetterCall::UpdateValueIndex(22, update),
        BinaryStream,
        RdbcStreamLength::Int(-23)
    );
    assert_stream_call!(
        result_set.update_binary_stream_by_label_with_int_length(
            &mut connection,
            "binary_int",
            Some(&input),
            24,
        ),
        |update| StrongGetterCall::UpdateValueLabel("binary_int".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Int(24)
    );
    assert_stream_call!(
        result_set.update_binary_stream_with_length(&mut connection, 23, Some(&input), -25),
        |update| StrongGetterCall::UpdateValueIndex(23, update),
        BinaryStream,
        RdbcStreamLength::Long(-25)
    );
    assert_stream_call!(
        result_set.update_binary_stream_by_label_with_length(
            &mut connection,
            "binary_long",
            Some(&input),
            26,
        ),
        |update| StrongGetterCall::UpdateValueLabel("binary_long".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Long(26)
    );

    macro_rules! assert_reader_call {
        ($expression:expr, $target:expr, $variant:ident, $length:expr) => {
            assert_last_update!(
                $expression,
                $target(ResultSetUpdate::$variant {
                    reader: Some(reader.clone()),
                    length: $length,
                })
            );
        };
    }
    assert_reader_call!(
        result_set.update_character_stream(&mut connection, 24, Some(&reader)),
        |update| StrongGetterCall::UpdateValueIndex(24, update),
        CharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_character_stream_by_label(&mut connection, "character", Some(&reader)),
        |update| StrongGetterCall::UpdateValueLabel("character".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set
            .update_character_stream_with_int_length(&mut connection, 25, Some(&reader), -27,),
        |update| StrongGetterCall::UpdateValueIndex(25, update),
        CharacterStream,
        RdbcCharacterLength::Int(-27)
    );
    assert_reader_call!(
        result_set.update_character_stream_by_label_with_int_length(
            &mut connection,
            "character_int",
            Some(&reader),
            28,
        ),
        |update| StrongGetterCall::UpdateValueLabel("character_int".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Int(28)
    );
    assert_reader_call!(
        result_set.update_character_stream_with_length(&mut connection, 26, Some(&reader), -29,),
        |update| StrongGetterCall::UpdateValueIndex(26, update),
        CharacterStream,
        RdbcCharacterLength::Long(-29)
    );
    assert_reader_call!(
        result_set.update_character_stream_by_label_with_length(
            &mut connection,
            "character_long",
            Some(&reader),
            30,
        ),
        |update| StrongGetterCall::UpdateValueLabel("character_long".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Long(30)
    );
    assert_reader_call!(
        result_set.update_n_character_stream(&mut connection, 27, Some(&reader)),
        |update| StrongGetterCall::UpdateValueIndex(27, update),
        NCharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_n_character_stream_by_label(
            &mut connection,
            "n_character",
            Some(&reader)
        ),
        |update| StrongGetterCall::UpdateValueLabel("n_character".to_string(), update),
        NCharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_n_character_stream_with_length(&mut connection, 28, Some(&reader), -31,),
        |update| StrongGetterCall::UpdateValueIndex(28, update),
        NCharacterStream,
        RdbcCharacterLength::Long(-31)
    );
    assert_reader_call!(
        result_set.update_n_character_stream_by_label_with_length(
            &mut connection,
            "n_character_long",
            Some(&reader),
            32,
        ),
        |update| StrongGetterCall::UpdateValueLabel("n_character_long".to_string(), update),
        NCharacterStream,
        RdbcCharacterLength::Long(32)
    );

    assert_eq!(probe.calls().len(), 56);
    assert!(!input.is_closed());
    assert!(!reader.is_closed());
}

#[tokio::test]
async fn statement_close_closes_traced_result_set_and_records_fetched_peak() {
    let mut connection = sqlite_pooled_connection().await;
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(
            &mut connection,
            "SELECT 1 UNION ALL SELECT 2 UNION ALL SELECT 3",
        )
        .await
        .unwrap();

    assert!(result_set.next(&mut connection).unwrap());
    assert!(result_set.next(&mut connection).unwrap());
    statement.close_with_connection(&mut connection).unwrap();

    assert!(statement.is_closed());
    assert!(result_set.is_closed());
    assert!(result_set.raw_result_set().is_closed());
    assert_eq!(statement.fetch_row_peak(), 2);
    assert!(matches!(
        result_set.next(&mut connection),
        Err(DruidError::Other(message)) if message == "result set is closed"
    ));
}

#[tokio::test]
async fn result_set_errors_typed_getters_and_old_lease_are_observable() {
    let returned: Arc<Mutex<Option<Box<dyn PhysicalConnection>>>> = Arc::new(Mutex::new(None));
    let returned_from_callback = Arc::clone(&returned);
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let physical = factory.create().await.unwrap();
    let mut connection = DruidPooledConnection::new(
        physical,
        19,
        Box::new(move |physical, _| {
            *returned_from_callback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(physical);
        }),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1")
        .await
        .unwrap();

    assert!(matches!(
        result_set.object(&mut connection, 1),
        Err(DruidError::Other(message))
            if message == "result set cursor is not positioned on a row"
    ));
    assert!(matches!(
        result_set.object_typed(&mut connection, 1, &RdbcTargetType::Custom("X".to_string())),
        Err(DruidError::Other(message))
            if message == "result set cursor is not positioned on a row"
    ));
    assert!(result_set.next(&mut connection).unwrap());
    assert!(matches!(
        result_set.object_typed(&mut connection, 1, &RdbcTargetType::Custom("X".to_string())),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_get_object_typed_custom"
        })
    ));
    assert_eq!(statement.exception_count(), 3);

    connection.close().await.unwrap();
    let physical = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("连接归还回调必须收到物理连接");
    let mut next_lease = DruidPooledConnection::new(physical, 19, Box::new(|_, _| {}));
    assert!(matches!(
        result_set.next(&mut next_lease),
        Err(DruidError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn labeled_result_set_preserves_all_scalar_and_stream_overloads() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let physical: Arc<dyn PhysicalResultSet> = Arc::new(RowSetResultSet::with_column_labels(
        vec![Row::new(vec![
            Value::Int(7),
            Value::Float(2.5),
            Value::Bytes(vec![9, 8]),
            Value::String("labelled".to_string()),
        ])],
        vec![
            "number".to_string(),
            "score".to_string(),
            "payload".to_string(),
            "label".to_string(),
        ],
    ));
    let mut result_set = statement.wrap_result_set(physical).unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set
            .short_by_label(&mut connection, "number")
            .unwrap(),
        7
    );
    assert_eq!(
        result_set.byte_by_label(&mut connection, "number").unwrap(),
        7
    );
    assert_eq!(
        result_set.float_by_label(&mut connection, "score").unwrap(),
        2.5
    );
    assert_eq!(
        result_set
            .bytes_by_label(&mut connection, "payload")
            .unwrap(),
        Some(vec![9, 8])
    );
    assert_eq!(
        result_set
            .n_string_by_label(&mut connection, "label")
            .unwrap(),
        Some("labelled".to_string())
    );
    assert_eq!(
        result_set
            .binary_stream_by_label(&mut connection, "payload")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        vec![9, 8]
    );
    assert_eq!(
        result_set
            .ascii_stream_by_label(&mut connection, "label")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"labelled"
    );
    assert_eq!(
        result_set
            .unicode_stream_by_label(&mut connection, "label")
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"labelled"
    );
    assert_eq!(
        result_set
            .character_stream_by_label(&mut connection, "label")
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "labelled"
    );
    assert_eq!(
        result_set
            .n_character_stream_by_label(&mut connection, "label")
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "labelled"
    );
}

#[tokio::test]
async fn strong_typed_getters_preserve_all_decimal_temporal_and_calendar_overloads() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let decimal = BigDecimal::from_str("1234567890.45").unwrap();
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(13, 14, 15, 123_456_789).unwrap();
    let timestamp = NaiveDateTime::new(date, time);
    let physical: Arc<dyn PhysicalResultSet> = Arc::new(RowSetResultSet::with_column_labels(
        vec![Row::new(vec![
            Value::Decimal(decimal.clone()),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(timestamp),
            Value::Null,
        ])],
        vec![
            "amount".to_string(),
            "event_date".to_string(),
            "event_time".to_string(),
            "event_at".to_string(),
            "optional_amount".to_string(),
        ],
    ));
    let mut result_set = statement.wrap_result_set(physical).unwrap();
    assert!(result_set.next(&mut connection).unwrap());

    let meta_data = result_set.meta_data(&mut connection).unwrap();
    assert_eq!(
        meta_data.column_type(1).unwrap(),
        ResultSetColumnType::Decimal
    );
    assert_eq!(meta_data.column_type(2).unwrap(), ResultSetColumnType::Date);
    assert_eq!(meta_data.column_type(3).unwrap(), ResultSetColumnType::Time);
    assert_eq!(
        meta_data.column_type(4).unwrap(),
        ResultSetColumnType::Timestamp
    );

    assert_eq!(
        result_set.big_decimal(&mut connection, 1).unwrap(),
        Some(decimal.clone())
    );
    assert_eq!(
        result_set
            .big_decimal_by_label(&mut connection, "amount")
            .unwrap(),
        Some(decimal.clone())
    );
    assert_eq!(
        result_set
            .big_decimal_with_scale(&mut connection, 1, 2)
            .unwrap(),
        Some(decimal.clone())
    );
    assert_eq!(
        result_set
            .big_decimal_by_label_with_scale(&mut connection, "amount", 2)
            .unwrap(),
        Some(decimal)
    );

    let shanghai = RdbcCalendar::new("Asia/Shanghai").unwrap();
    assert_eq!(result_set.date(&mut connection, 2).unwrap(), Some(date));
    assert_eq!(
        result_set
            .date_by_label(&mut connection, "event_date")
            .unwrap(),
        Some(date)
    );
    assert_eq!(
        result_set
            .date_with_calendar(&mut connection, 2, Some(shanghai.clone()))
            .unwrap(),
        Some(date)
    );
    assert_eq!(
        result_set
            .date_by_label_with_calendar(&mut connection, "event_date", None)
            .unwrap(),
        Some(date)
    );

    assert_eq!(result_set.time(&mut connection, 3).unwrap(), Some(time));
    assert_eq!(
        result_set
            .time_by_label(&mut connection, "event_time")
            .unwrap(),
        Some(time)
    );
    assert_eq!(
        result_set
            .time_with_calendar(&mut connection, 3, None)
            .unwrap(),
        Some(time)
    );
    assert_eq!(
        result_set
            .time_by_label_with_calendar(&mut connection, "event_time", Some(shanghai.clone()))
            .unwrap(),
        Some(time)
    );

    assert_eq!(
        result_set.timestamp(&mut connection, 4).unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label(&mut connection, "event_at")
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_with_calendar(&mut connection, 4, Some(shanghai))
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label_with_calendar(&mut connection, "event_at", None)
            .unwrap(),
        Some(timestamp)
    );

    assert_eq!(
        result_set
            .big_decimal_by_label(&mut connection, "optional_amount")
            .unwrap(),
        None
    );
    assert!(result_set.was_null(&mut connection).unwrap());
}

#[tokio::test]
async fn strong_typed_getter_conversion_errors_follow_statement_check_exception_path() {
    let mut connection = sqlite_pooled_connection().await;
    let statement = connection.create_statement().await.unwrap();
    let physical: Arc<dyn PhysicalResultSet> = Arc::new(RowSetResultSet::with_column_labels(
        vec![Row::new(vec![Value::String("not-a-date".to_string())])],
        vec!["event_date".to_string()],
    ));
    let mut result_set = statement.wrap_result_set(physical).unwrap();
    assert!(result_set.next(&mut connection).unwrap());
    assert!(matches!(
        result_set.date_by_label(&mut connection, "event_date"),
        Err(DruidError::DriverError(message))
            if message.contains("cannot be converted to Date")
    ));
    assert!(matches!(
        result_set.blob(&mut connection, 1),
        Err(DruidError::UnsupportedOperation {
            operation: "result_set_blob"
        })
    ));
    assert_eq!(statement.exception_count(), 2);
}

#[test]
fn row_set_result_set_preserves_labels_navigation_and_validation() {
    let result_set = RowSetResultSet::with_column_labels(
        vec![
            Row::new(vec![Value::Int(1), Value::Null]),
            Row::new(vec![Value::Int(2), Value::String("two".to_string())]),
        ],
        vec!["ID".to_string(), "label".to_string()],
    );

    assert!(result_set.is_before_first().unwrap());
    assert_eq!(result_set.find_column("id").unwrap(), 1);
    assert!(result_set.first().unwrap());
    assert_eq!(result_set.row().unwrap(), 1);
    assert_eq!(result_set.value(2).unwrap(), Value::Null);
    assert!(result_set.was_null().unwrap());
    assert!(result_set.last().unwrap());
    assert_eq!(result_set.row().unwrap(), 2);
    assert!(result_set.absolute(-2).unwrap());
    assert_eq!(result_set.row().unwrap(), 1);
    assert!(result_set.relative(1).unwrap());
    assert_eq!(result_set.row().unwrap(), 2);
    result_set.before_first().unwrap();
    assert!(result_set.next().unwrap());
    result_set.after_last().unwrap();
    assert!(result_set.is_after_last().unwrap());
    assert!(result_set.previous().unwrap());

    result_set.set_fetch_direction(1001).unwrap();
    assert_eq!(result_set.fetch_direction().unwrap(), 1001);
    result_set.set_fetch_size(32).unwrap();
    assert_eq!(result_set.fetch_size().unwrap(), 32);
    assert_eq!(result_set.result_set_type().unwrap(), 1004);
    assert_eq!(result_set.concurrency().unwrap(), 1007);
    assert_eq!(result_set.holdability().unwrap(), 1);

    assert!(matches!(
        result_set.find_column("missing"),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        result_set.value(0),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        result_set.set_fetch_direction(999),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        result_set.set_fetch_size(-1),
        Err(DruidError::InvalidArgument(_))
    ));
    result_set.close().unwrap();
    assert!(result_set.is_closed());
    assert!(matches!(
        result_set.next(),
        Err(DruidError::Other(message)) if message == "result set is closed"
    ));
    assert!(matches!(
        result_set.meta_data(),
        Err(DruidError::Other(message)) if message == "result set is closed"
    ));
}

fn scalar_default_row_set() -> RowSetResultSet {
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_opt(13, 14, 15).unwrap();
    RowSetResultSet::with_column_labels(
        vec![Row::new(vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(2.5),
            Value::Decimal(BigDecimal::from_str("3.25").unwrap()),
            Value::String("17".to_string()),
            Value::Bytes(b"bytes".to_vec()),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(NaiveDateTime::new(date, time)),
            Value::Bytes(vec![0xff]),
            Value::String("not-a-number".to_string()),
            Value::Int(i64::MAX),
            Value::Bytes(vec![1, 2]),
        ])],
        (1..=14).map(|index| format!("c{index}")).collect(),
    )
}

fn assert_default_scalar_successes(result_set: &RowSetResultSet) {
    assert_eq!(result_set.string(1).unwrap(), None);
    for index in 2..=10 {
        assert!(result_set.string(index).unwrap().is_some());
    }
    assert_eq!(
        result_set.string_by_label("c3").unwrap().as_deref(),
        Some("42")
    );
    assert!(!result_set.boolean(1).unwrap());
    assert!(result_set.boolean(2).unwrap());
    assert!(result_set.boolean(3).unwrap());
    assert!(result_set.boolean(4).unwrap());
    assert!(result_set.boolean(5).unwrap());
    assert!(!result_set.boolean_by_label("c6").unwrap());
    assert_eq!(result_set.long(1).unwrap(), 0);
    assert_eq!(result_set.long(2).unwrap(), 1);
    assert_eq!(result_set.long(3).unwrap(), 42);
    assert_eq!(result_set.long(4).unwrap(), 2);
    assert_eq!(result_set.long(5).unwrap(), 3);
    assert_eq!(result_set.long_by_label("c6").unwrap(), 17);
    assert_eq!(result_set.int_by_label("c3").unwrap(), 42);
    assert_eq!(result_set.short_by_label("c3").unwrap(), 42);
    assert_eq!(result_set.byte_by_label("c3").unwrap(), 42);
    assert_eq!(result_set.double(1).unwrap(), 0.0);
    assert_eq!(result_set.double(2).unwrap(), 1.0);
    assert_eq!(result_set.double(3).unwrap(), 42.0);
    assert_eq!(result_set.double(4).unwrap(), 2.5);
    assert_eq!(result_set.double(5).unwrap(), 3.25);
    assert_eq!(result_set.double_by_label("c6").unwrap(), 17.0);
    assert_eq!(result_set.float_by_label("c4").unwrap(), 2.5);
    assert_eq!(result_set.bytes(1).unwrap(), None);
    assert_eq!(result_set.bytes(7).unwrap(), Some(b"bytes".to_vec()));
    assert_eq!(
        result_set.bytes_by_label("c3").unwrap(),
        Some(b"42".to_vec())
    );
    assert_eq!(result_set.value_by_label("c3").unwrap(), Value::Int(42));
}

fn assert_default_scalar_failures(result_set: &RowSetResultSet) {
    assert!(matches!(
        result_set.string(11),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.boolean(7),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.long(12),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.long(14),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.double(12),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.double(14),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.int(13),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.short(13),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.byte(13),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.value(15),
        Err(DruidError::InvalidArgument(message)) if message.contains("exceeds row width")
    ));
}

fn assert_default_stream_conversions(result_set: &RowSetResultSet) {
    assert!(result_set.binary_stream(1).unwrap().is_none());
    assert!(result_set.character_stream(1).unwrap().is_none());
    for index in 2..=10 {
        assert!(result_set.binary_stream(index).unwrap().is_some());
        assert!(result_set.character_stream(index).unwrap().is_some());
    }
    assert!(matches!(
        result_set.character_stream(11),
        Err(DruidError::DriverError(_))
    ));
}

#[test]
fn row_set_default_getters_cover_null_label_conversion_and_stream_contracts() {
    let result_set = scalar_default_row_set();
    assert!(result_set.next().unwrap());
    assert_default_scalar_successes(&result_set);
    assert_default_scalar_failures(&result_set);
    assert_default_stream_conversions(&result_set);

    assert!(!result_set.absolute(0).unwrap());
    assert!(result_set.absolute(1).unwrap());

    let physical: Arc<dyn PhysicalResultSet> = Arc::new(result_set);
    let first = druid_core::core::RdbcResultSet::new(physical.clone());
    let same = first.clone();
    let other = druid_core::core::RdbcResultSet::new(Arc::new(SparsePhysicalResultSet));
    assert_eq!(first, same);
    assert_ne!(first, other);
    assert!(std::ptr::eq(first.physical(), physical.as_ref()));
    assert!(format!("{first:?}").contains("RdbcResultSet"));
    first.close().unwrap();
    assert!(first.is_closed());
}

fn typed_conversion_row_set() -> RowSetResultSet {
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(13, 14, 15, 123_000_000).unwrap();
    RowSetResultSet::new(vec![Row::new(vec![
        Value::Null,
        Value::Int(42),
        Value::Float(2.5),
        Value::Float(f64::NAN),
        Value::String("3.75".to_string()),
        Value::String("invalid".to_string()),
        Value::Bytes(vec![1]),
        Value::Date(date),
        Value::Time(time),
        Value::Timestamp(NaiveDateTime::new(date, time)),
        Value::String("2026-07-30".to_string()),
        Value::String("2026-07-30 01:02:03.456".to_string()),
        Value::String("2026-07-30T01:02:03.456".to_string()),
        Value::String("01:02:03.456".to_string()),
    ])])
}

fn assert_default_decimal_and_date_conversions(result_set: &RowSetResultSet) {
    assert_eq!(result_set.big_decimal(1, None).unwrap(), None);
    assert_eq!(
        result_set.big_decimal(2, None).unwrap(),
        Some(BigDecimal::from(42))
    );
    assert_eq!(
        result_set.big_decimal(3, Some(1)).unwrap(),
        Some(BigDecimal::from_str("2.5").unwrap())
    );
    assert_eq!(
        result_set.big_decimal(5, None).unwrap(),
        Some(BigDecimal::from_str("3.75").unwrap())
    );
    assert!(matches!(
        result_set.big_decimal(4, None),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.big_decimal(6, None),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.big_decimal(7, None),
        Err(DruidError::DriverError(_))
    ));

    assert_eq!(
        result_set
            .date(1, &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        None
    );
    assert!(result_set
        .date(10, &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert!(result_set
        .date(11, &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert!(matches!(
        result_set.date(6, &RdbcCalendarArgument::Unspecified),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.date(7, &RdbcCalendarArgument::Unspecified),
        Err(DruidError::DriverError(_))
    ));
}

fn assert_default_time_and_timestamp_conversions(result_set: &RowSetResultSet) {
    assert_eq!(
        result_set
            .time(1, &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        None
    );
    assert!(result_set
        .time(10, &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert!(result_set
        .time(14, &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert!(matches!(
        result_set.time(6, &RdbcCalendarArgument::Unspecified),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.time(7, &RdbcCalendarArgument::Unspecified),
        Err(DruidError::DriverError(_))
    ));

    assert_eq!(
        result_set
            .timestamp(1, &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        None
    );
    for index in [8, 12, 13] {
        assert!(result_set
            .timestamp(index, &RdbcCalendarArgument::Unspecified)
            .unwrap()
            .is_some());
    }
    assert!(matches!(
        result_set.timestamp(6, &RdbcCalendarArgument::Unspecified),
        Err(DruidError::DriverError(_))
    ));
    assert!(matches!(
        result_set.timestamp(7, &RdbcCalendarArgument::Unspecified),
        Err(DruidError::DriverError(_))
    ));
}

#[test]
fn default_typed_conversions_cover_alternate_values_and_driver_errors() {
    let result_set = typed_conversion_row_set();
    assert!(result_set.next().unwrap());
    assert_default_decimal_and_date_conversions(&result_set);
    assert_default_time_and_timestamp_conversions(&result_set);
}

#[test]
fn sql_warning_and_result_set_metadata_preserve_structured_fields() {
    let mut warning = SqlWarning::new("first", Some("01000".to_string()), 7);
    warning.set_next_warning(SqlWarning::new("second", None, 8));
    assert_eq!(warning.message(), "first");
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 7);
    assert_eq!(warning.next_warning().unwrap().message(), "second");

    let result_set = RowSetResultSet::with_column_labels(
        vec![Row::new(vec![Value::Bool(true), Value::Null])],
        vec!["enabled".to_string(), "optional".to_string()],
    );
    let meta_data = result_set.meta_data().unwrap();
    assert_eq!(meta_data.column_label(1).unwrap(), "enabled");
    assert_eq!(
        meta_data.column_type(1).unwrap(),
        ResultSetColumnType::Boolean
    );
    assert_eq!(
        meta_data.column_type(2).unwrap(),
        ResultSetColumnType::Unknown
    );
    assert!(!meta_data.is_nullable(1).unwrap());
    assert!(meta_data.is_nullable(2).unwrap());
    assert!(matches!(
        meta_data.column_label(0),
        Err(DruidError::InvalidArgument(_))
    ));
    assert!(matches!(
        meta_data.column_type(3),
        Err(DruidError::InvalidArgument(_))
    ));
}

#[test]
fn result_set_metadata_preserves_complete_rdbc_column_contract() {
    let column = ResultSetColumnMeta::new("amount_alias", ResultSetColumnType::Decimal, true)
        .with_origin("amount", "app", "invoice", "main")
        .with_type_identity("MONEY", "com.example.Money")
        .with_shape(24, 19, 4)
        .with_nullability(ResultSetNullability::Unknown)
        .with_flags(true, false, true, true, true, false, true, true);
    let meta_data = druid_core::core::ResultSetMetaData::new(vec![column]);

    assert_eq!(meta_data.column_count().unwrap(), 1);
    assert_eq!(meta_data.column_label(1).unwrap(), "amount_alias");
    assert_eq!(meta_data.column_name(1).unwrap(), "amount");
    assert_eq!(meta_data.schema_name(1).unwrap(), "app");
    assert_eq!(meta_data.table_name(1).unwrap(), "invoice");
    assert_eq!(meta_data.catalog_name(1).unwrap(), "main");
    assert_eq!(
        meta_data.column_type(1).unwrap(),
        ResultSetColumnType::Decimal
    );
    assert_eq!(meta_data.rdbc_type(1).unwrap(), 3);
    assert_eq!(meta_data.column_type_name(1).unwrap(), "MONEY");
    assert_eq!(meta_data.column_class_name(1).unwrap(), "com.example.Money");
    assert_eq!(
        meta_data.nullability(1).unwrap(),
        ResultSetNullability::Unknown
    );
    assert_eq!(meta_data.nullable_code(1).unwrap(), 2);
    assert!(!meta_data.is_nullable(1).unwrap());
    assert!(meta_data.is_auto_increment(1).unwrap());
    assert!(!meta_data.is_case_sensitive(1).unwrap());
    assert!(meta_data.is_searchable(1).unwrap());
    assert!(meta_data.is_currency(1).unwrap());
    assert!(meta_data.is_signed(1).unwrap());
    assert_eq!(meta_data.column_display_size(1).unwrap(), 24);
    assert_eq!(meta_data.precision(1).unwrap(), 19);
    assert_eq!(meta_data.scale(1).unwrap(), 4);
    assert!(!meta_data.is_read_only(1).unwrap());
    assert!(meta_data.is_writable(1).unwrap());
    assert!(meta_data.is_definitely_writable(1).unwrap());

    let all_types = [
        (
            ResultSetColumnType::Unknown,
            1_111,
            "OTHER",
            "java.lang.Object",
            false,
        ),
        (
            ResultSetColumnType::Boolean,
            16,
            "BOOLEAN",
            "java.lang.Boolean",
            false,
        ),
        (
            ResultSetColumnType::Integer,
            -5,
            "BIGINT",
            "java.lang.Long",
            true,
        ),
        (
            ResultSetColumnType::Float,
            8,
            "DOUBLE",
            "java.lang.Double",
            true,
        ),
        (
            ResultSetColumnType::Decimal,
            3,
            "DECIMAL",
            "java.math.BigDecimal",
            true,
        ),
        (
            ResultSetColumnType::Date,
            91,
            "DATE",
            "java.sql.Date",
            false,
        ),
        (
            ResultSetColumnType::Time,
            92,
            "TIME",
            "java.sql.Time",
            false,
        ),
        (
            ResultSetColumnType::Timestamp,
            93,
            "TIMESTAMP",
            "java.sql.Timestamp",
            false,
        ),
        (
            ResultSetColumnType::Text,
            12,
            "VARCHAR",
            "java.lang.String",
            false,
        ),
        (ResultSetColumnType::Binary, -3, "VARBINARY", "[B", false),
    ];
    for (column_type, rdbc_type, type_name, class_name, signed) in all_types {
        assert_eq!(column_type.rdbc_type(), rdbc_type);
        assert_eq!(column_type.type_name(), type_name);
        assert_eq!(column_type.class_name(), class_name);
        assert_eq!(column_type.is_signed(), signed);
    }
    assert_eq!(ResultSetNullability::NoNulls.rdbc_code(), 0);
    assert_eq!(ResultSetNullability::Nullable.rdbc_code(), 1);
    assert_eq!(ResultSetNullability::Unknown.rdbc_code(), 2);
}

#[test]
fn physical_result_set_metadata_preserves_getter_error_and_wrapper_identity() {
    let probe = Arc::new(PhysicalMetaDataProbe::new(None));
    let physical: Arc<dyn PhysicalResultSetMetaData> = probe.clone();
    let meta_data = ResultSetMetaData::from_physical(physical);

    assert_eq!(meta_data.column_count().unwrap(), 1);
    assert!(meta_data.is_auto_increment(1).unwrap());
    assert!(meta_data.is_case_sensitive(1).unwrap());
    assert!(meta_data.is_searchable(1).unwrap());
    assert!(meta_data.is_currency(1).unwrap());
    assert!(meta_data.is_nullable(1).unwrap());
    assert_eq!(
        meta_data.nullability(1).unwrap(),
        ResultSetNullability::Nullable
    );
    assert_eq!(meta_data.nullable_code(1).unwrap(), 1);
    assert!(meta_data.is_signed(1).unwrap());
    assert_eq!(meta_data.column_display_size(1).unwrap(), 21);
    assert_eq!(meta_data.column_label(1).unwrap(), "label_1");
    assert_eq!(meta_data.column_name(1).unwrap(), "name_1");
    assert_eq!(meta_data.schema_name(1).unwrap(), "schema_1");
    assert_eq!(meta_data.precision(1).unwrap(), 31);
    assert_eq!(meta_data.scale(1).unwrap(), 5);
    assert_eq!(meta_data.table_name(1).unwrap(), "table_1");
    assert_eq!(meta_data.catalog_name(1).unwrap(), "catalog_1");
    assert_eq!(
        meta_data.column_type(1).unwrap(),
        ResultSetColumnType::Decimal
    );
    assert_eq!(meta_data.rdbc_type(1).unwrap(), 3);
    assert_eq!(meta_data.column_type_name(1).unwrap(), "MONEY_1");
    assert!(!meta_data.is_read_only(1).unwrap());
    assert!(meta_data.is_writable(1).unwrap());
    assert!(meta_data.is_definitely_writable(1).unwrap());
    assert_eq!(
        meta_data.column_class_name(1).unwrap(),
        "com.example.Money1"
    );
    assert_eq!(
        probe.calls(),
        vec![
            "column_count",
            "is_auto_increment",
            "is_case_sensitive",
            "is_searchable",
            "is_currency",
            "nullability",
            "nullability",
            "nullability",
            "is_signed",
            "column_display_size",
            "column_label",
            "column_name",
            "schema_name",
            "precision",
            "scale",
            "table_name",
            "catalog_name",
            "column_type",
            "column_type",
            "column_type_name",
            "is_read_only",
            "is_writable",
            "is_definitely_writable",
            "column_class_name",
        ]
    );

    assert!(meta_data.physical().is_some());
    assert!(meta_data.is_wrapper_for_type::<ResultSetMetaData>());
    assert!(meta_data.is_wrapper_for_type::<dyn PhysicalResultSetMetaData>());
    assert!(meta_data.is_wrapper_for_type::<PhysicalMetaDataProbe>());
    assert!(std::ptr::eq(
        meta_data
            .unwrap_ref::<PhysicalMetaDataProbe>()
            .expect("必须解包到底层具体 metadata"),
        probe.as_ref()
    ));
    assert!(std::ptr::eq(
        meta_data
            .unwrap(Some(TypeId::of::<dyn PhysicalResultSetMetaData>()))
            .and_then(|value| value.result_set_meta_data())
            .expect("必须解包到物理 metadata SPI"),
        probe.as_ref() as &dyn PhysicalResultSetMetaData
    ));
    assert_eq!(meta_data, meta_data.clone());
    assert!(format!("{meta_data:?}").contains("PhysicalMetaDataProbe"));

    let failing_probe = Arc::new(PhysicalMetaDataProbe::new(Some("column_label")));
    let failing = ResultSetMetaData::from_physical(failing_probe.clone());
    assert!(matches!(
        failing.column_label(9),
        Err(DruidError::DriverError(message))
            if message == "metadata probe failed at column_label"
    ));
    assert_eq!(failing_probe.calls(), vec!["column_label"]);
    assert_ne!(meta_data, failing);
    assert_ne!(meta_data, ResultSetMetaData::default());
}
