//! 结果集诊断：filter_overloads_a 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 完整保留原 NString 默认重载、短路、错误计数和真实 Unicode 合同。
#[tokio::test]
async fn task6_pooled_result_set_preserves_distinct_n_string_filter_overloads() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }

    let physical_log = Arc::new(Mutex::new(Vec::new()));
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(pass_chain))).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut physical_probe = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(n_string_read_probe::NStringReadProbe::new(Arc::clone(
            &physical_log,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    assert_eq!(
        physical_probe
            .n_string(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Some("索引-NString".to_string())
    );
    assert_eq!(
        physical_probe.n_string_by_label("name").unwrap(),
        Some("标签-NString".to_string())
    );
    assert_eq!(
        calls(&physical_log),
        ["physical:n_string:1", "physical:n_string_by_label:name"]
    );
    physical_probe.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(
        n_string_short_circuit_filter::NStringShortCircuitFilter {
            calls: Arc::clone(&call_log),
        },
    ));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filter_chain))).await;
    let mut result_set = statement.execute_query("SELECT '原值'").await.unwrap();
    assert_eq!(
        result_set.n_string(ColumnIndex::new(3).unwrap()).unwrap(),
        Some("过滤-索引".to_string())
    );
    assert_eq!(
        result_set.n_string_by_label("name").unwrap(),
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
        result_set.n_string_by_label("fail"),
        Err(RdbcError::DriverError(
            "filtered NString failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // 真实 Toasty SQLite Unicode 值通过默认委托的独立 NString 入口读取。
    let mut pass_chain = FilterChain::new();
    pass_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(pass_chain))).await;
    let mut result_set = statement
        .execute_query("SELECT '你好，Druid'")
        .await
        .unwrap();
    assert!(result_set.next().await.unwrap());
    assert_eq!(
        result_set.n_string(ColumnIndex::new(1).unwrap()).unwrap(),
        Some("你好，Druid".to_string())
    );
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// NString 的独立短路同样必须服从隐藏列与关闭边界。
#[tokio::test]
async fn task6_n_string_filter_short_circuit_obeys_hidden_columns_and_closed_guards() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(
        n_string_short_circuit_filter::NStringShortCircuitFilter {
            calls: Arc::clone(&log),
        },
    ));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT '原值' AS value",
            Some(Arc::new(filters)),
        )
        .await;
    rows.set_hidden_columns(Some(vec![1]));
    assert_eq!(
        rows.n_string_by_label("value"),
        Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
    );
    assert!(log.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 1);
    rows.set_hidden_columns(None);
    assert_eq!(
        rows.n_string_by_label("fail"),
        Err(RdbcError::DriverError(
            "filtered NString failure".to_owned()
        ))
    );
    assert_eq!(statement.exception_count(), 2);
    assert_eq!(
        *log.lock().unwrap(),
        vec!["result_set_get_n_string_by_label:fail"]
    );
    rows.close().await.unwrap();
    assert_eq!(
        rows.n_string_by_label("virtual"),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert_eq!(
        rows.n_string(ColumnIndex::new(3).unwrap()),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert_eq!(statement.exception_count(), 2);
    assert_eq!(log.lock().unwrap().len(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_result_set_preserves_decimal_temporal_and_calendar_filter_overloads() {
    use super::strong_getter_short_circuit_filter::StrongGetterShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError};
    use bigdecimal::BigDecimal;
    use chrono::{NaiveDate, NaiveTime};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::{RdbcCalendar, RdbcCalendarArgument};
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StrongGetterShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filter_chain))).await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();
    assert_eq!(
        result_set
            .big_decimal(ColumnIndex::new(1).unwrap())
            .unwrap(),
        Some(BigDecimal::from(101))
    );
    assert_eq!(
        result_set.big_decimal_by_label("decimal").unwrap(),
        Some(BigDecimal::from(102))
    );
    assert_eq!(
        result_set
            .big_decimal_with_scale(ColumnIndex::new(2).unwrap(), 7)
            .unwrap(),
        Some(BigDecimal::from(103))
    );
    assert_eq!(
        result_set
            .big_decimal_by_label_with_scale("decimal_scale", 8)
            .unwrap(),
        Some(BigDecimal::from(104))
    );

    assert_eq!(
        result_set
            .date(
                ColumnIndex::new(3).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 1).unwrap())
    );
    assert_eq!(
        result_set
            .date_by_label("date", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 2).unwrap())
    );
    assert_eq!(
        result_set
            .date(
                ColumnIndex::new(4).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 3).unwrap())
    );
    assert_eq!(
        result_set
            .date_by_label(
                "date_calendar",
                &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
            )
            .unwrap(),
        Some(NaiveDate::from_ymd_opt(2031, 1, 3).unwrap())
    );

    assert_eq!(
        result_set
            .time(
                ColumnIndex::new(5).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(1, 2, 3).unwrap())
    );
    assert_eq!(
        result_set
            .time_by_label("time", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(NaiveTime::from_hms_opt(2, 3, 4).unwrap())
    );
    assert_eq!(
        result_set
            .time(
                ColumnIndex::new(6).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(NaiveTime::from_hms_opt(3, 4, 5).unwrap())
    );
    assert_eq!(
        result_set
            .time_by_label(
                "time_calendar",
                &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
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
        result_set
            .timestamp(
                ColumnIndex::new(7).unwrap(),
                &RdbcCalendarArgument::Unspecified
            )
            .unwrap(),
        Some(index_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label("timestamp", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(label_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp(
                ColumnIndex::new(8).unwrap(),
                &RdbcCalendarArgument::Specified(None)
            )
            .unwrap(),
        Some(calendar_timestamp)
    );
    assert_eq!(
        result_set
            .timestamp_by_label(
                "timestamp_calendar",
                &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
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
        result_set.timestamp_by_label(
            "fail",
            &RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap()))
        ),
        Err(RdbcError::DriverError(
            "filtered temporal failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_temporal_filter_short_circuit_obeys_hidden_columns_and_closed_guards() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::RdbcCalendarArgument;
    let log = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(
        strong_getter_short_circuit_filter::StrongGetterShortCircuitFilter {
            calls: Arc::clone(&log),
        },
    ));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT 1 AS value",
            Some(Arc::new(filters)),
        )
        .await;
    rows.set_hidden_columns(Some(vec![1]));
    assert_eq!(
        rows.timestamp_by_label("value", &RdbcCalendarArgument::Specified(None)),
        Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
    );
    assert!(log.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 1);
    rows.set_hidden_columns(None);
    assert_eq!(
        rows.timestamp_by_label("fail", &RdbcCalendarArgument::Specified(None)),
        Err(RdbcError::DriverError(
            "filtered temporal failure".to_owned()
        ))
    );
    assert_eq!(statement.exception_count(), 2);
    assert_eq!(
        *log.lock().unwrap(),
        vec!["result_set_get_timestamp_by_label_with_calendar:fail:specified:null"]
    );
    rows.close().await.unwrap();
    assert_eq!(
        rows.timestamp_by_label("virtual", &RdbcCalendarArgument::Specified(None)),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert_eq!(statement.exception_count(), 2);
    assert_eq!(log.lock().unwrap().len(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity() {
    use super::object_short_circuit_filter::ObjectShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError, Value};
    use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap};
    use std::sync::Mutex;
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let argument_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ObjectShortCircuitFilter {
        calls: Arc::clone(&call_log),
        arguments: Arc::clone(&argument_log),
    }));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let filters = Some(Arc::new(filter_chain));
    let policy = StandardLogicalPolicy::new(
        75,
        "sqlite-object-filter".to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        filters.clone(),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let raw_statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut statement = DruidPooledStatement::new_standard(
        raw_statement,
        StatementOptions::default(),
        Arc::clone(&resources),
        policy,
        75,
        1,
        Arc::new(AtomicU64::new(1)),
        Arc::new(AtomicU64::new(1)),
        lease_active,
        filters,
        None,
    );
    resources
        .register(&admission, Arc::clone(&statement.inner))
        .await
        .unwrap();
    drop(admission);
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

    let empty_map = RdbcTypeMap::new();
    let mut populated_map = RdbcTypeMap::new();
    populated_map.insert(
        "example.address",
        RdbcTargetType::Custom("example.Address".to_string()),
    );
    let custom_target = RdbcTargetType::Custom("example.Target".to_string());
    let string_target = RdbcTargetType::String;

    assert_eq!(
        result_set.object(ColumnIndex::new(1).unwrap()).unwrap(),
        RdbcObject::Scalar(Value::Int(901))
    );
    assert_eq!(
        result_set.object_by_label("value").unwrap(),
        RdbcObject::Scalar(Value::String("filtered-label-object".to_string()))
    );
    assert_eq!(
        result_set
            .object_with_type_map(ColumnIndex::new(2).unwrap(), None)
            .unwrap(),
        RdbcObject::String("filtered-map:null".to_string())
    );
    assert_eq!(
        result_set
            .object_with_type_map(ColumnIndex::new(3).unwrap(), Some(&empty_map))
            .unwrap(),
        RdbcObject::String("filtered-map:empty".to_string())
    );
    assert_eq!(
        result_set
            .object_with_type_map_by_label("mapped", Some(&populated_map),)
            .unwrap(),
        RdbcObject::String(
            "filtered-label-map:example.address=Custom(\"example.Address\")".to_string()
        )
    );
    assert_eq!(
        result_set
            .object_as(ColumnIndex::new(4).unwrap(), &custom_target)
            .unwrap(),
        RdbcObject::String("filtered-typed:Custom(\"example.Target\")".to_string())
    );
    assert_eq!(
        result_set
            .object_as_by_label("typed", &string_target)
            .unwrap(),
        RdbcObject::String("filtered-label-typed:String".to_string())
    );

    assert_eq!(
        call_log.lock().unwrap().clone(),
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

    // 补充观察原映射和目标类型引用，不能只比较重新构造后的内容。
    assert_eq!(
        *argument_log.lock().unwrap(),
        vec![
            std::ptr::from_ref(&empty_map) as usize,
            std::ptr::from_ref(&populated_map) as usize,
            std::ptr::from_ref(&custom_target) as usize,
            std::ptr::from_ref(&string_target) as usize
        ],
    );
    assert_eq!(
        result_set.object_as_by_label("fail", &RdbcTargetType::Custom("example.Fail".to_string()),),
        Err(RdbcError::DriverError(
            "filtered object failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
