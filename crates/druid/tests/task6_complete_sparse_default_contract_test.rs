//! 验证真实标准默认实现，不以 fixture 覆盖模拟原始能力拒绝。
#[path = "sparse_default_support/sparse_default_result_set.rs"]
mod sparse_default_result_set;

use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::FetchDirection;
use druid_rdbc::types::{RdbcCalendarArgument, RdbcTargetType};
use sparse_default_result_set::SparseDefaultResultSet;

fn task6_assert_unsupported_operation<T: std::fmt::Debug>(
    result: Result<T, RdbcError>,
    expected_operation: &'static str,
) {
    assert!(
        matches!(result, Err(RdbcError::UnsupportedOperation { operation })
            if operation == expected_operation),
        "expected unsupported operation {expected_operation}"
    );
}

/// 保留原十个默认能力调用与精确错误名称。
/// 对应 Java: java.sql.ResultSet 的导航与值读取合同。
#[tokio::test]
async fn task6_sparse_result_set_navigation_defaults_report_exact_capabilities() {
    let mut result_set = SparseDefaultResultSet;
    task6_assert_unsupported_operation(result_set.next().await, "result_set_next");
    task6_assert_unsupported_operation(result_set.previous().await, "result_set_previous");
    task6_assert_unsupported_operation(result_set.first().await, "result_set_first");
    task6_assert_unsupported_operation(result_set.last().await, "result_set_last");
    task6_assert_unsupported_operation(result_set.before_first().await, "result_set_before_first");
    task6_assert_unsupported_operation(result_set.after_last().await, "result_set_after_last");
    task6_assert_unsupported_operation(result_set.absolute(-2).await, "result_set_absolute");
    task6_assert_unsupported_operation(result_set.relative(3).await, "result_set_relative");
    task6_assert_unsupported_operation(result_set.row(), "result_set_row");
    task6_assert_unsupported_operation(
        result_set.value(ColumnIndex::new(1).unwrap()),
        "result_set_value",
    );
}

/// 保留原三十一项默认属性与精确能力错误。
/// 对应 Java: java.sql.ResultSet 默认属性合同。
#[test]
fn task6_sparse_result_set_property_defaults_report_exact_capabilities() {
    let mut result_set = SparseDefaultResultSet;
    task6_assert_unsupported_operation(result_set.find_column("id"), "result_set_find_column");
    task6_assert_unsupported_operation(result_set.was_null(), "result_set_was_null");
    task6_assert_unsupported_operation(
        result_set.ascii_stream(ColumnIndex::new(1).unwrap()),
        "result_set_ascii_stream",
    );
    task6_assert_unsupported_operation(
        result_set.unicode_stream(ColumnIndex::new(1).unwrap()),
        "result_set_unicode_stream",
    );
    task6_assert_unsupported_operation(
        result_set.binary_stream(ColumnIndex::new(1).unwrap()),
        "result_set_binary_stream",
    );
    task6_assert_unsupported_operation(
        result_set.character_stream(ColumnIndex::new(1).unwrap()),
        "result_set_character_stream",
    );
    task6_assert_unsupported_operation(result_set.is_before_first(), "result_set_is_before_first");
    task6_assert_unsupported_operation(result_set.is_after_last(), "result_set_is_after_last");
    task6_assert_unsupported_operation(result_set.is_first(), "result_set_is_first");
    task6_assert_unsupported_operation(result_set.is_last(), "result_set_is_last");
    task6_assert_unsupported_operation(
        result_set.set_fetch_direction(FetchDirection::Forward),
        "result_set_set_fetch_direction",
    );
    task6_assert_unsupported_operation(result_set.fetch_direction(), "result_set_fetch_direction");
    task6_assert_unsupported_operation(result_set.set_fetch_size(10), "result_set_set_fetch_size");
    task6_assert_unsupported_operation(result_set.fetch_size(), "result_set_fetch_size");
    task6_assert_unsupported_operation(result_set.result_set_type(), "result_set_type");
    task6_assert_unsupported_operation(result_set.concurrency(), "result_set_concurrency");
    task6_assert_unsupported_operation(result_set.holdability(), "result_set_holdability");
    task6_assert_unsupported_operation(result_set.warnings(), "result_set_warnings");
    task6_assert_unsupported_operation(result_set.clear_warnings(), "result_set_clear_warnings");
    task6_assert_unsupported_operation(result_set.cursor_name(), "result_set_cursor_name");
    task6_assert_unsupported_operation(result_set.meta_data(), "result_set_meta_data");
    task6_assert_unsupported_operation(result_set.row_updated(), "result_set_row_updated");
    task6_assert_unsupported_operation(result_set.row_inserted(), "result_set_row_inserted");
    task6_assert_unsupported_operation(result_set.row_deleted(), "result_set_row_deleted");
    task6_assert_unsupported_operation(result_set.insert_row(), "result_set_insert_row");
    task6_assert_unsupported_operation(result_set.update_row(), "result_set_update_row");
    task6_assert_unsupported_operation(result_set.delete_row(), "result_set_delete_row");
    task6_assert_unsupported_operation(result_set.refresh_row(), "result_set_refresh_row");
    task6_assert_unsupported_operation(
        result_set.cancel_row_updates(),
        "result_set_cancel_row_updates",
    );
    task6_assert_unsupported_operation(
        result_set.move_to_insert_row(),
        "result_set_move_to_insert_row",
    );
    task6_assert_unsupported_operation(
        result_set.move_to_current_row(),
        "result_set_move_to_current_row",
    );
}

/// 分别观察所有已知默认差异，防止首个失败掩盖其他错误种类。
/// 对应 Java: java.sql.ResultSet 的默认能力拒绝。
#[test]
fn task6_sparse_property_default_errors_are_all_exact() {
    let mut result_set = SparseDefaultResultSet;
    let column = ColumnIndex::new(1).unwrap();
    let observations = vec![
        (
            result_set.ascii_stream(column).map(|_| ()),
            "result_set_ascii_stream",
        ),
        (
            result_set.unicode_stream(column).map(|_| ()),
            "result_set_unicode_stream",
        ),
        (
            result_set.binary_stream(column).map(|_| ()),
            "result_set_binary_stream",
        ),
        (
            result_set.character_stream(column).map(|_| ()),
            "result_set_character_stream",
        ),
        (result_set.result_set_type().map(|_| ()), "result_set_type"),
        (
            result_set.concurrency().map(|_| ()),
            "result_set_concurrency",
        ),
        (
            result_set.row_updated().map(|_| ()),
            "result_set_row_updated",
        ),
        (
            result_set.row_inserted().map(|_| ()),
            "result_set_row_inserted",
        ),
        (
            result_set.row_deleted().map(|_| ()),
            "result_set_row_deleted",
        ),
        (result_set.insert_row(), "result_set_insert_row"),
        (result_set.update_row(), "result_set_update_row"),
        (result_set.delete_row(), "result_set_delete_row"),
        (result_set.refresh_row(), "result_set_refresh_row"),
        (
            result_set.cancel_row_updates(),
            "result_set_cancel_row_updates",
        ),
        (
            result_set.move_to_insert_row(),
            "result_set_move_to_insert_row",
        ),
        (
            result_set.move_to_current_row(),
            "result_set_move_to_current_row",
        ),
    ];
    let failures = observations.into_iter().filter_map(|(actual, expected)| {
        if matches!(&actual, Err(RdbcError::UnsupportedOperation { operation }) if *operation == expected) {
            None
        } else {
            Some(format!("{expected}: {actual:?}"))
        }
    }).collect::<Vec<_>>();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// 缓冲后端自身支持三个行状态查询，并保持关闭后的原精确错误。
/// 对应 Java: RowSetResultSet#rowUpdated/rowInserted/rowDeleted。
#[tokio::test]
async fn task6_buffered_row_status_preserves_native_capabilities_and_closed_errors() {
    let mut result_set = druid_rdbc::extensions::BufferedRowSet::new(Vec::new());
    assert!(!result_set.row_updated().unwrap());
    assert!(!result_set.row_inserted().unwrap());
    assert!(!result_set.row_deleted().unwrap());
    result_set.close().await.unwrap();
    for actual in [
        result_set.row_updated(),
        result_set.row_inserted(),
        result_set.row_deleted(),
    ] {
        assert!(
            matches!(actual, Err(RdbcError::Other(message)) if message == "result set is closed")
        );
    }
}

/// 保留原十二个类型化调用及值/列名/类型映射错误优先级。
/// 对应 Java: java.sql.ResultSet 类型化默认读取合同。
#[test]
fn task6_sparse_result_set_typed_defaults_preserve_error_priority() {
    let result_set = SparseDefaultResultSet;
    let calendar = RdbcCalendarArgument::Unspecified;
    task6_assert_unsupported_operation(
        task6_sparse_big_decimal(&result_set, 1, None),
        "result_set_value",
    );
    task6_assert_unsupported_operation(
        task6_sparse_big_decimal_by_label(&result_set, "amount", Some(2)),
        "result_set_find_column",
    );
    task6_assert_unsupported_operation(
        result_set.date(ColumnIndex::new(1).unwrap(), &calendar),
        "result_set_value",
    );
    task6_assert_unsupported_operation(
        result_set.date_by_label("date", &calendar),
        "result_set_find_column",
    );
    task6_assert_unsupported_operation(
        result_set.time(ColumnIndex::new(1).unwrap(), &calendar),
        "result_set_value",
    );
    task6_assert_unsupported_operation(
        result_set.time_by_label("time", &calendar),
        "result_set_find_column",
    );
    task6_assert_unsupported_operation(
        result_set.timestamp(ColumnIndex::new(1).unwrap(), &calendar),
        "result_set_value",
    );
    task6_assert_unsupported_operation(
        result_set.timestamp_by_label("timestamp", &calendar),
        "result_set_find_column",
    );
    task6_assert_unsupported_operation(
        result_set.object_with_type_map(ColumnIndex::new(1).unwrap(), None),
        "result_set_get_object_with_type_map",
    );
    task6_assert_unsupported_operation(
        result_set.object_by_label_with_type_map("object", None),
        "result_set_get_object_by_label_with_type_map",
    );
    task6_assert_unsupported_operation(
        result_set.object_as(
            ColumnIndex::new(1).unwrap(),
            &RdbcTargetType::Custom("vendor.Type".to_string()),
        ),
        "result_set_value",
    );
    task6_assert_unsupported_operation(
        result_set.object_by_label_as("object", &RdbcTargetType::Custom("vendor.Type".to_string())),
        "result_set_find_column",
    );
}

fn task6_sparse_big_decimal(
    result_set: &SparseDefaultResultSet,
    column_index: usize,
    scale: Option<i32>,
) -> Result<Option<bigdecimal::BigDecimal>, RdbcError> {
    let column = ColumnIndex::new(column_index)?;
    match scale {
        Some(scale) => result_set.big_decimal_with_scale(column, scale),
        None => result_set.big_decimal(column),
    }
}

fn task6_sparse_big_decimal_by_label(
    result_set: &SparseDefaultResultSet,
    label: &str,
    scale: Option<i32>,
) -> Result<Option<bigdecimal::BigDecimal>, RdbcError> {
    match scale {
        Some(scale) => result_set.big_decimal_by_label_with_scale(label, scale),
        None => result_set.big_decimal_by_label(label),
    }
}
