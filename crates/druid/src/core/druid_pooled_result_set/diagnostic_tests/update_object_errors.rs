//! 结果集诊断：update_object_errors 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

#[test]
fn task6_result_set_scalar_update_filter_defaults_delegate_all_java_overloads() {
    use super::update_call::UpdateCall;
    use crate::core::{FilterAdapter, FilterChain, ResultSetFilterContext};
    use druid_rdbc::result_set::ResultSetUpdate;
    use druid_rdbc::types::{RdbcBigDecimal, RdbcNaiveDate, RdbcNaiveTime};
    use std::str::FromStr;
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut raw = update_identity_probe::UpdateIdentityProbe::new(Arc::clone(&calls));
    let context = ResultSetFilterContext::new();
    let mut chain = FilterChain::new();
    chain.add_result_set(Arc::new(FilterAdapter::new()));
    let decimal = RdbcBigDecimal::from_str("12.340").unwrap();
    let date = RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = RdbcNaiveTime::from_hms_opt(10, 11, 12).unwrap();
    let timestamp = date.and_time(time);
    // 完整保留 BASE 的14种setter及下标/原列名28次调用，默认Filter继续到驱动末端。
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(1).unwrap(),
            ResultSetUpdate::Null,
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "null_value",
            ResultSetUpdate::Null,
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(2).unwrap(),
            ResultSetUpdate::Boolean(true),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "boolean_value",
            ResultSetUpdate::Boolean(false),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(3).unwrap(),
            ResultSetUpdate::Byte(-8),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "byte_value",
            ResultSetUpdate::Byte(8),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(4).unwrap(),
            ResultSetUpdate::Short(-16),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "short_value",
            ResultSetUpdate::Short(16),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(5).unwrap(),
            ResultSetUpdate::Int(-32),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "int_value",
            ResultSetUpdate::Int(32),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(6).unwrap(),
            ResultSetUpdate::Long(-64),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "long_value",
            ResultSetUpdate::Long(64),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(7).unwrap(),
            ResultSetUpdate::Float(1.25),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "float_value",
            ResultSetUpdate::Float(-1.25),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(8).unwrap(),
            ResultSetUpdate::Double(2.5),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "double_value",
            ResultSetUpdate::Double(-2.5),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(9).unwrap(),
            ResultSetUpdate::BigDecimal(Some(decimal.clone())),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "decimal_value",
            ResultSetUpdate::BigDecimal(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(10).unwrap(),
            ResultSetUpdate::String(Some("index".to_owned())),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "string_value",
            ResultSetUpdate::String(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(11).unwrap(),
            ResultSetUpdate::Bytes(Some(vec![0, 255])),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "bytes_value",
            ResultSetUpdate::Bytes(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(12).unwrap(),
            ResultSetUpdate::Date(Some(date)),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "date_value",
            ResultSetUpdate::Date(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(13).unwrap(),
            ResultSetUpdate::Time(Some(time)),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "time_value",
            ResultSetUpdate::Time(None),
        )
        .unwrap();
    chain
        .standard_result_set_update(
            &mut raw,
            &context,
            ColumnIndex::new(14).unwrap(),
            ResultSetUpdate::Timestamp(Some(timestamp)),
        )
        .unwrap();
    chain
        .standard_result_set_update_by_label(
            &mut raw,
            &context,
            "timestamp_value",
            ResultSetUpdate::Timestamp(None),
        )
        .unwrap();
    let observed = calls.lock().unwrap();
    assert_eq!(observed.len(), 28);
    assert_eq!(observed[0], UpdateCall::Index(1, ResultSetUpdate::Null));
    assert_eq!(
        observed[1],
        UpdateCall::Label("null_value".to_owned(), ResultSetUpdate::Null)
    );
    assert_eq!(
        observed[2],
        UpdateCall::Index(2, ResultSetUpdate::Boolean(true))
    );
    assert_eq!(
        observed[3],
        UpdateCall::Label("boolean_value".to_owned(), ResultSetUpdate::Boolean(false))
    );
    assert_eq!(
        format!("{:?}", observed[16]),
        "Index(9, BigDecimal(Some(BigDecimal(sign=Plus, scale=3, digits=[12340]))))"
    );
    assert_eq!(
        observed[17],
        UpdateCall::Label(
            "decimal_value".to_owned(),
            ResultSetUpdate::BigDecimal(None)
        )
    );
    assert_eq!(
        observed[20],
        UpdateCall::Index(11, ResultSetUpdate::Bytes(Some(vec![0, 255])))
    );
    assert_eq!(
        observed[27],
        UpdateCall::Label(
            "timestamp_value".to_owned(),
            ResultSetUpdate::Timestamp(None)
        )
    );
}

#[tokio::test]
async fn task6_strong_typed_getter_conversion_errors_follow_statement_check_exception_path() {
    use druid_rdbc::extensions::BufferedRowSet;
    use druid_rdbc::types::{RdbcCalendarArgument, Row, Value};
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(BufferedRowSet::with_column_labels(
            vec![Row::new(vec![Value::String("not-a-date".to_string())])],
            vec!["event_date".to_string()],
        )),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    assert!(result_set.next().await.unwrap());
    let error = result_set
        .date_by_label("event_date", &RdbcCalendarArgument::Unspecified)
        .unwrap_err();
    assert_eq!(error.class_name(), "java.sql.SQLDataException");
    assert_eq!(error.sql_exception().unwrap().sql_state(), Some("22007"));
    assert!(!error.to_string().contains("not-a-date"));
    assert!(matches!(
        result_set.blob(ColumnIndex::new(1).unwrap()),
        Err(crate::core::RdbcError::UnsupportedOperation {
            operation: "result_set_blob"
        })
    ));
    assert_eq!(statement.exception_count(), 2);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_map_get_object_overloads_preserve_index_label_and_nullable_map_identity() {
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap, Value};
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let probe = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(strong_getter_probe::StrongGetterProbe::new(Arc::clone(
            &probe,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let mut type_map = RdbcTypeMap::new();
    type_map.insert("APP.MONEY", RdbcTargetType::BigDecimal);
    assert_eq!(
        result_set
            .object_with_type_map(ColumnIndex::new(31).unwrap(), Some(&type_map))
            .unwrap(),
        RdbcObject::Scalar(Value::Int(71))
    );
    assert_eq!(
        result_set
            .object_by_label_with_type_map("mapped", None)
            .unwrap(),
        RdbcObject::Scalar(Value::Int(72))
    );
    assert_eq!(
        *probe.lock().unwrap(),
        vec![
            strong_getter_call::StrongGetterCall::ObjectMapIndex(31, Some(type_map)),
            strong_getter_call::StrongGetterCall::ObjectMapLabel("mapped".to_owned(), None),
        ]
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_typed_get_object_overloads_preserve_raw_target_type_identity() {
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcObject, RdbcTargetType};
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let probe = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(strong_getter_probe::StrongGetterProbe::new(Arc::clone(
            &probe,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    assert_eq!(
        result_set
            .object_as(
                ColumnIndex::new(41).unwrap(),
                &RdbcTargetType::Custom("com.example.Money".to_owned())
            )
            .unwrap(),
        RdbcObject::String("typed-index".to_owned())
    );
    assert_eq!(
        result_set
            .object_by_label_as("payload", &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::String("typed-label".to_owned())
    );
    assert_eq!(
        *probe.lock().unwrap(),
        vec![
            strong_getter_call::StrongGetterCall::ObjectTypedIndex(
                41,
                RdbcTargetType::Custom("com.example.Money".to_owned())
            ),
            strong_getter_call::StrongGetterCall::ObjectTypedLabel(
                "payload".to_owned(),
                RdbcTargetType::Bytes
            ),
        ]
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn object_label_overloads_keep_dynamic_dispatch_visibility_maps_and_closed_guards() {
    use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap, Value};
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let probe = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(strong_getter_probe::StrongGetterProbe::new(Arc::clone(
            &probe,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    rows.set_hidden_columns(Some(vec![2]));
    assert!(rows.object_with_type_map_by_label("mapped", None).is_err());
    assert!(rows
        .object_as_by_label("mapped", &RdbcTargetType::Bytes)
        .is_err());
    assert!(probe.lock().unwrap().is_empty());
    rows.set_hidden_columns(None);
    rows.set_logic_column_map(Some(HashMap::from([(1, 2)])));
    rows.set_physical_column_map(Some(HashMap::from([(2, 1)])));
    let mut map = RdbcTypeMap::new();
    map.insert("APP.MONEY", RdbcTargetType::BigDecimal);
    let mut standard: Box<dyn ResultSet> = Box::new(rows);
    assert_eq!(
        standard
            .object_with_type_map_by_label("mapped", Some(&map))
            .unwrap(),
        RdbcObject::Scalar(Value::Int(72))
    );
    assert_eq!(
        standard
            .object_as_by_label("mapped", &RdbcTargetType::Bytes)
            .unwrap(),
        RdbcObject::String("typed-label".to_owned())
    );
    assert_eq!(
        standard
            .object_as(ColumnIndex::new(1).unwrap(), &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::String("typed-index".to_owned())
    );
    assert!(standard
        .object_with_type_map_by_label("missing", None)
        .is_err());
    assert!(standard
        .object_as(ColumnIndex::new(99).unwrap(), &RdbcTargetType::Bytes)
        .is_err());
    assert_eq!(
        *probe.lock().unwrap(),
        vec![
            strong_getter_call::StrongGetterCall::ObjectMapLabel("mapped".to_owned(), Some(map)),
            strong_getter_call::StrongGetterCall::ObjectTypedLabel(
                "mapped".to_owned(),
                RdbcTargetType::Bytes
            ),
            strong_getter_call::StrongGetterCall::ObjectTypedIndex(2, RdbcTargetType::String),
        ]
    );
    standard.close().await.unwrap();
    assert_eq!(
        standard.object_with_type_map_by_label("mapped", None),
        Err(crate::core::RdbcError::ConnectionDiscarded)
    );
    assert_eq!(
        standard.object_as_by_label("mapped", &RdbcTargetType::Bytes),
        Err(crate::core::RdbcError::ConnectionDiscarded)
    );
    assert_eq!(probe.lock().unwrap().len(), 3);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
