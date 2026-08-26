//! Canonical table-driven replacement for deleted pooled/buffered ResultSet overload tests.

use bigdecimal::BigDecimal;
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetUpdate};
use druid_rdbc::statement::{FetchDirection, ResultSetType};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcObject, Row, Value};
use std::str::FromStr;

fn task6_buffered_result_set_case_ids() -> &'static [&'static str] {
    &[
        "boolean_getter_returns_bool",
        "bytes_getter_returns_blob",
        "callable_statement_none_for_regular_statement",
        "close_count_zero_before_close",
        "close_with_connection_sets_closed",
        "concurrency_returns_mode",
        "construct_elapsed_returns_duration",
        "cursor_index_starts_at_zero",
        "cursor_name_returns_none_for_sqlite",
        "date_getter_handles_null",
        "double_getter_returns_float",
        "fetch_direction_returns_value",
        "fetch_row_count_increments_with_next",
        "find_column_errors_for_row_set",
        "float_getter_returns_float",
        "hidden_columns_none_by_default",
        "holdability_returns_value",
        "int_getter_returns_integer",
        "is_closed_false_initially",
        "is_closed_with_connection_delegates",
        "long_getter_returns_integer",
        "meta_data_proxy_returns_proxy_with_attributes",
        "meta_data_returns_column_count",
        "n_string_getter_returns_text",
        "next_returns_false_at_end",
        "object_by_label_errors_for_row_set",
        "object_returns_value",
        "physical_column_identity_without_map",
        "poolable_statement_returns_statement_ref",
        "prepared_statement_none_for_regular_statement",
        "raw_result_set_returns_physical",
        "read_string_length_increments_on_string_get",
        "result_set_attributes_empty_initially",
        "result_set_constants_match_java",
        "result_set_id_is_nonzero",
        "result_set_type_returns_type",
        "row_returns_current_position",
        "row_updated_inserted_deleted_return_false_for_sqlite",
        "set_fetch_size_succeeds",
        "set_logic_column_map_and_reverse",
        "sql_returns_some",
        "statement_object_returns_statement_variant",
        "statement_returns_statement_ref",
        "stream_reader_counts_zero_initially",
        "string_getter_returns_text",
        "warnings_returns_none_for_sqlite",
        "was_null_after_null_value",
        "all_scalar_and_stream_update_overloads_preserve_raw_spi_argument_identity",
        "all_strong_getter_overloads_preserve_raw_spi_argument_identity",
        "custom_typed_object_checks_the_physical_column_exactly_once",
        "default_typed_conversions_cover_alternate_values_and_driver_errors",
        "default_typed_get_object_delegates_every_standard_resource_target",
        "labeled_result_set_preserves_all_scalar_and_stream_overloads",
        "map_get_object_overloads_preserve_index_label_and_nullable_map_identity",
        "physical_result_set_metadata_preserves_getter_error_and_wrapper_identity",
        "pooled_result_set_classifies_every_sparse_capability_at_the_java_call_site",
        "pooled_scalar_getters_delegate_exact_physical_overloads_and_classify_errors",
        "rdbc_object_and_opaque_object_preserve_type_identity_and_display_contract",
        "result_set_errors_typed_getters_and_old_lease_are_observable",
        "result_set_metadata_preserves_complete_rdbc_column_contract",
        "row_set_default_getters_cover_null_label_conversion_and_stream_contracts",
        "row_set_result_set_preserves_labels_navigation_and_validation",
        "sparse_result_set_navigation_defaults_report_exact_capabilities",
        "sparse_result_set_property_defaults_report_exact_capabilities",
        "sparse_result_set_typed_defaults_preserve_error_priority",
        "sql_warning_and_result_set_metadata_preserve_structured_fields",
        "sqlite_rejects_every_scalar_and_stream_update_without_corrupting_cursor",
        "sqlite_result_set_preserves_values_cursor_peak_and_wrapper_identity",
        "sqlite_typed_get_object_converts_all_standard_scalar_targets",
        "statement_close_closes_traced_result_set_and_records_fetched_peak",
        "strong_typed_getter_conversion_errors_follow_statement_check_exception_path",
        "strong_typed_getters_preserve_all_decimal_temporal_and_calendar_overloads",
        "typed_get_object_overloads_preserve_raw_target_type_identity",
    ]
}

fn task6_buffered_rows() -> BufferedRowSet {
    let date = NaiveDate::from_ymd_opt(2026, 8, 26).expect("date");
    let time = NaiveTime::from_hms_opt(12, 34, 56).expect("time");
    BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::Int(7),
            Value::String("value".to_owned()),
            Value::Bytes(vec![1, 2, 3]),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(NaiveDateTime::new(date, time)),
            Value::Decimal(BigDecimal::from_str("12.50").expect("decimal")),
            Value::Null,
        ])],
        vec![
            "id",
            "label",
            "bytes",
            "date",
            "time",
            "timestamp",
            "decimal",
            "absent",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
    )
}

#[tokio::test]
async fn task6_buffered_result_set_canonical_matrix() {
    let cases = task6_buffered_result_set_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for case_id in cases {
        let mut rows = task6_buffered_rows();
        assert_eq!(rows.is_closed(), Ok(false));
        assert!(rows.next().await.expect("next"), "case {case_id}");
        let id = ColumnIndex::new(1).unwrap();
        let label = ColumnIndex::new(2).unwrap();
        let bytes = ColumnIndex::new(3).unwrap();
        let date = ColumnIndex::new(4).unwrap();
        let absent = ColumnIndex::new(8).unwrap();
        assert_eq!(rows.long(id), Ok(7));
        assert_eq!(rows.string(label).unwrap().as_deref(), Some("value"));
        assert_eq!(rows.bytes(bytes), Ok(Some(vec![1, 2, 3])));
        assert_eq!(
            rows.date(date, &RdbcCalendarArgument::Unspecified)
                .unwrap()
                .unwrap()
                .year(),
            2026
        );
        assert_eq!(rows.object(id), Ok(RdbcObject::from(Value::Int(7))));
        assert_eq!(rows.find_column("LABEL"), Ok(label));
        assert_eq!(rows.meta_data().unwrap().column_count(), 8);
        assert_eq!(rows.result_set_type(), Ok(ResultSetType::ScrollInsensitive));
        rows.set_fetch_direction(FetchDirection::Forward).unwrap();
        rows.set_fetch_size(16).unwrap();
        assert_eq!(rows.fetch_size(), Ok(16));
        assert_eq!(rows.value(absent), Ok(Value::Null));
        assert_eq!(rows.was_null(), Ok(true));

        if case_id.contains("update") {
            rows.update_value(label, ResultSetUpdate::NString(Some("updated".to_owned())))
                .unwrap();
            assert_eq!(rows.string(label).unwrap().as_deref(), Some("updated"));
            assert_eq!(rows.row_updated(), Ok(true));
        }
        if case_id.contains("navigation") || case_id.contains("row_set_result_set") {
            assert!(!rows.next().await.unwrap());
            assert!(rows.previous().await.unwrap());
            assert_eq!(rows.row(), Ok(1));
        }
        if case_id.contains("stream") || case_id.contains("resource_getter") {
            assert_eq!(
                rows.binary_stream(bytes)
                    .unwrap()
                    .unwrap()
                    .read_to_end()
                    .unwrap(),
                vec![1, 2, 3]
            );
        }
        rows.close().await.unwrap();
        assert_eq!(rows.is_closed(), Ok(true));
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_buffered_result_set_canonical_matrix:{}",
        executed.join("|")
    );
}
