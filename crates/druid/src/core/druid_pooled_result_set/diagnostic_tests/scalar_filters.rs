//! 结果集诊断：scalar_filters 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 完整保留九类标量原重载、顺序、短路和精确错误计数；候选未运行。
#[tokio::test]
async fn task6_pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads() {
    use super::scalar_short_circuit_filter::ScalarShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ScalarShortCircuitFilter {
        calls: Arc::clone(&call_log),
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
        73,
        "sqlite-scalar-filter".to_owned(),
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
        73,
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

    // Filter 在 raw ResultSet 前短路，因此未调用 next 也能返回改写值。
    assert_eq!(
        result_set.string(ColumnIndex::new(1).unwrap()).unwrap(),
        Some("filtered-index".to_string())
    );
    assert_eq!(
        result_set.string_by_label("string").unwrap(),
        Some("filtered-label".to_string())
    );
    assert!(result_set.boolean(ColumnIndex::new(2).unwrap()).unwrap());
    assert!(!result_set.boolean_by_label("boolean").unwrap());
    assert_eq!(result_set.byte(ColumnIndex::new(3).unwrap()).unwrap(), 11);
    assert_eq!(result_set.byte_by_label("byte").unwrap(), 12);
    assert_eq!(result_set.short(ColumnIndex::new(4).unwrap()).unwrap(), 21);
    assert_eq!(result_set.short_by_label("short").unwrap(), 22);
    assert_eq!(result_set.int(ColumnIndex::new(5).unwrap()).unwrap(), 31);
    assert_eq!(result_set.int_by_label("int").unwrap(), 32);
    assert_eq!(result_set.long(ColumnIndex::new(6).unwrap()).unwrap(), 41);
    assert_eq!(result_set.long_by_label("long").unwrap(), 42);
    assert_eq!(
        result_set.float(ColumnIndex::new(7).unwrap()).unwrap(),
        51.5
    );
    assert_eq!(result_set.float_by_label("float").unwrap(), 52.5);
    assert_eq!(
        result_set.double(ColumnIndex::new(8).unwrap()).unwrap(),
        61.5
    );
    assert_eq!(result_set.double_by_label("double").unwrap(), 62.5);
    assert_eq!(
        result_set.bytes(ColumnIndex::new(9).unwrap()).unwrap(),
        Some(vec![7, 8])
    );
    assert_eq!(
        result_set.bytes_by_label("bytes").unwrap(),
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
        result_set.double_by_label("fail"),
        Err(RdbcError::DriverError(
            "filtered double failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 九类标量短路仍须服从全部下标/标签隐藏列、关闭及失效租约边界；候选未运行。
#[tokio::test]
async fn task6_scalar_filter_all_overloads_obey_hidden_closed_and_expired_lease_guards() {
    use super::scalar_short_circuit_filter::ScalarShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetRead};
    use std::sync::Mutex;

    let requests = [
        ResultSetRead::String,
        ResultSetRead::Boolean,
        ResultSetRead::Byte,
        ResultSetRead::Short,
        ResultSetRead::Int,
        ResultSetRead::Long,
        ResultSetRead::Float,
        ResultSetRead::Double,
        ResultSetRead::Bytes,
    ];
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(ScalarShortCircuitFilter {
        calls: Arc::clone(&log),
    }));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT 1 AS value",
            Some(Arc::new(filters)),
        )
        .await;
    rows.set_hidden_columns(Some(vec![1]));
    for request in &requests {
        assert_eq!(
            rows.read_object(ColumnIndex::new(1).unwrap(), request),
            Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
        );
        assert_eq!(
            rows.read_object_by_label("value", request),
            Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
        );
    }
    assert!(log.lock().unwrap().is_empty());
    // 沿完整对象治理合同，每个实际失败入口只向原创建语句记错一次。
    assert_eq!(statement.exception_count(), 18);
    rows.set_hidden_columns(None);
    assert_eq!(
        rows.double_by_label("fail"),
        Err(RdbcError::DriverError("filtered double failure".to_owned()))
    );
    assert_eq!(statement.exception_count(), 19);
    assert_eq!(
        log.lock().unwrap().as_slice(),
        ["result_set_get_double_by_label:fail"]
    );
    rows.close().await.unwrap();
    for request in &requests {
        assert_eq!(
            rows.read_object(ColumnIndex::new(1).unwrap(), request),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert_eq!(
            rows.read_object_by_label("value", request),
            Err(RdbcError::ConnectionDiscarded)
        );
    }
    assert_eq!(statement.exception_count(), 19);
    assert_eq!(log.lock().unwrap().len(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let expired_log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(ScalarShortCircuitFilter {
        calls: Arc::clone(&expired_log),
    }));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT 1 AS value",
            Some(Arc::new(filters)),
        )
        .await;
    statement.inner.lease_active.store(false, Ordering::Release);
    for request in &requests {
        assert_eq!(
            rows.read_object(ColumnIndex::new(1).unwrap(), request),
            Err(RdbcError::ConnectionDiscarded)
        );
        assert_eq!(
            rows.read_object_by_label("value", request),
            Err(RdbcError::ConnectionDiscarded)
        );
    }
    assert!(expired_log.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 九类原生默认委托及注册顺序/短路/错误回退；候选未运行。
#[tokio::test]
async fn task6_scalar_filter_preserves_all_native_overloads_order_short_circuit_and_error_unwind() {
    use super::ordered_scalar_filter::OrderedScalarFilter;
    use super::scalar_read_probe::{scalar_stem, ScalarReadProbe};
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetRead;
    use druid_rdbc::types::{RdbcObject, Value};
    use std::sync::Mutex;
    let requests = [
        ResultSetRead::String,
        ResultSetRead::Boolean,
        ResultSetRead::Byte,
        ResultSetRead::Short,
        ResultSetRead::Int,
        ResultSetRead::Long,
        ResultSetRead::Float,
        ResultSetRead::Double,
        ResultSetRead::Bytes,
    ];
    let expected = [
        Value::String("原生😀字符串".to_owned()),
        Value::Bool(true),
        Value::Int(11),
        Value::Int(21),
        Value::Int(31),
        Value::Int(41),
        Value::Float(51.5),
        Value::Float(61.5),
        Value::Bytes(vec![7, 8]),
    ];

    // 默认标签保留原生重载，不为下标value hook额外定位；完整read hook仍包围调用。
    let native_log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(FilterAdapter::new()));
    filters.add_result_set(Arc::new(FilterAdapter::new()));
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(ScalarReadProbe {
            calls: Arc::clone(&native_log),
            closed: false,
        }),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let mut expected_log = Vec::new();
    for (request, value) in requests.iter().zip(&expected) {
        assert_eq!(
            rows.read_object(ColumnIndex::new(7).unwrap(), request),
            Ok(RdbcObject::Scalar(value.clone()))
        );
        assert_eq!(
            rows.read_object_by_label("virtual", request),
            Ok(RdbcObject::Scalar(value.clone()))
        );
        let stem = scalar_stem(request);
        expected_log.extend([
            format!("raw:{stem}:7"),
            format!("raw:{stem}_by_label:virtual"),
        ]);
    }
    assert_eq!(*native_log.lock().unwrap(), expected_log);
    // 两次真实字符串成功读取按Java UTF-16长度计数，emoji占两个单元。
    assert_eq!(rows.filter_context.read_string_length(), 14);
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B"] {
        filters.add_result_set(Arc::new(OrderedScalarFilter {
            name,
            calls: Arc::clone(&log),
            short_circuit: false,
        }));
    }
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(ScalarReadProbe {
            calls: Arc::clone(&log),
            closed: false,
        }),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    for (request, value) in requests.iter().zip(&expected) {
        log.lock().unwrap().clear();
        assert_eq!(
            rows.read_object(ColumnIndex::new(7).unwrap(), request),
            Ok(RdbcObject::Scalar(value.clone()))
        );
        assert_eq!(
            *log.lock().unwrap(),
            [
                "A:before".to_owned(),
                "B:before".to_owned(),
                format!("raw:{}:7", scalar_stem(request)),
                "B:ok".to_owned(),
                "A:ok".to_owned()
            ]
        );
        log.lock().unwrap().clear();
        assert_eq!(
            rows.read_object_by_label("virtual", request),
            Ok(RdbcObject::Scalar(value.clone()))
        );
        assert_eq!(
            *log.lock().unwrap(),
            [
                "A:before".to_owned(),
                "B:before".to_owned(),
                format!("raw:{}_by_label:virtual", scalar_stem(request)),
                "B:ok".to_owned(),
                "A:ok".to_owned()
            ]
        );
    }
    log.lock().unwrap().clear();
    assert_eq!(
        rows.read_object_by_label("fail", &ResultSetRead::Double),
        Err(RdbcError::DriverError("scalar probe failure".to_owned()))
    );
    assert_eq!(
        *log.lock().unwrap(),
        [
            "A:before",
            "B:before",
            "raw:double_by_label:fail",
            "B:error",
            "A:error"
        ]
    );
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(rows.filter_context.read_string_length(), 14);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B", "C"] {
        filters.add_result_set(Arc::new(OrderedScalarFilter {
            name,
            calls: Arc::clone(&log),
            short_circuit: name == "B",
        }));
    }
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let mut rows = statement.execute_query("SELECT 1 AS value").await.unwrap();
    for request in &requests {
        for label in [None, Some("virtual")] {
            log.lock().unwrap().clear();
            let result = match label {
                Some(label) => rows.read_object_by_label(label, request),
                None => rows.read_object(ColumnIndex::new(7).unwrap(), request),
            };
            assert_eq!(result, Ok(RdbcObject::Scalar(Value::Null)));
            assert_eq!(
                *log.lock().unwrap(),
                ["A:before", "B:before", "B:ok", "A:ok"]
            );
        }
    }
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 完整创建语句/替换对象身份与错误合同；通过双向状态变更观察共享逻辑身份。
#[tokio::test]
async fn task6_pooled_result_set_routes_dynamic_statement_identity_through_filter() {
    use super::statement_short_circuit_filter::StatementShortCircuitFilter;
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::statement::{CallableStatement, PreparedStatement};
    use std::any::TypeId;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let replacement = Arc::new(Mutex::new(None));
    let fail = Arc::new(AtomicBool::new(false));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(StatementShortCircuitFilter {
        calls: Arc::clone(&call_log),
        replacement: Arc::clone(&replacement),
        fail: Arc::clone(&fail),
    }));
    filter_chain.add_result_set(Arc::new(FilterAdapter::new()));
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let mut connection = factory.create().await.unwrap();
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let filters = Some(Arc::new(filter_chain));
    let policy = StandardLogicalPolicy::new(
        84,
        "sqlite-statement-filter".to_owned(),
        Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)),
        filters.clone(),
        Some(true),
        generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let mut statements = Vec::new();
    for id in [1, 2] {
        let raw = connection
            .create_statement(StatementOptions::default())
            .await
            .unwrap();
        let statement = DruidPooledStatement::new_standard(
            raw,
            StatementOptions::default(),
            Arc::clone(&resources),
            Arc::clone(&policy),
            84,
            id,
            Arc::new(AtomicU64::new(1)),
            Arc::new(AtomicU64::new(1)),
            Arc::clone(&lease_active),
            filters.clone(),
            None,
        );
        resources
            .register(&admission, Arc::clone(&statement.inner))
            .await
            .unwrap();
        statements.push(statement);
    }
    drop(admission);
    let mut source_statement = statements.pop().unwrap();
    let mut replacement_statement = statements.pop().unwrap();
    let mut result_set = source_statement.execute_query("SELECT 1").await.unwrap();
    source_statement.set_fetch_size(11).unwrap();
    replacement_statement.set_fetch_size(31).unwrap();
    let mut returned = result_set.get_statement().unwrap().unwrap();
    assert!(returned
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert_eq!(returned.fetch_size().unwrap(), 11);
    returned.set_fetch_size(17).unwrap();
    assert_eq!(source_statement.fetch_size().unwrap(), 17);
    assert!(!returned
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert!(!returned
        .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap());
    assert!(!returned.is_closed().unwrap());
    drop(returned);
    replacement
        .lock()
        .unwrap()
        .replace(replacement_statement.clone());
    let mut returned = result_set.get_statement().unwrap().unwrap();
    assert!(returned
        .is_wrapper_for(Some(TypeId::of::<dyn Statement>()))
        .unwrap());
    assert_eq!(returned.fetch_size().unwrap(), 31);
    returned.set_fetch_size(37).unwrap();
    assert_eq!(replacement_statement.fetch_size().unwrap(), 37);
    assert_eq!(source_statement.fetch_size().unwrap(), 17);
    source_statement.set_fetch_size(19).unwrap();
    assert_eq!(returned.fetch_size().unwrap(), 37);
    assert!(!returned
        .is_wrapper_for(Some(TypeId::of::<dyn PreparedStatement>()))
        .unwrap());
    assert!(!returned
        .is_wrapper_for(Some(TypeId::of::<dyn CallableStatement>()))
        .unwrap());
    assert!(!returned.is_closed().unwrap());
    assert!(!source_statement.is_closed());
    assert!(!replacement_statement.is_closed());
    drop(returned);
    fail.store(true, Ordering::Release);
    assert_eq!(
        result_set.get_statement().map(|_| ()),
        Err(RdbcError::DriverError(
            "filtered statement failure".to_owned()
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
    fail.store(false, Ordering::Release);
    result_set.close().await.unwrap();
    source_statement.close().await.unwrap();
    replacement_statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 原逻辑所有者的关闭身份继续可查；旧租约必须在Filter前拒绝。
#[tokio::test]
async fn task6_statement_filter_order_closed_identity_and_expired_lease_are_observable() {
    use super::ordered_statement_filter::OrderedStatementFilter;
    use crate::core::{FilterChain, RdbcError};
    use std::sync::Mutex;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B"] {
        filters.add_result_set(Arc::new(OrderedStatementFilter {
            name,
            calls: Arc::clone(&calls),
            short_circuit: false,
            fail: false,
        }));
    }
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_filters(Some(Arc::new(filters))).await;
    let mut returned = rows.get_statement().unwrap().unwrap();
    returned.set_fetch_size(43).unwrap();
    assert_eq!(statement.fetch_size().unwrap(), 43);
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
    );
    drop(returned);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    calls.lock().unwrap().clear();
    let returned = rows.get_statement().unwrap().unwrap();
    assert!(returned.is_closed().unwrap());
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
    );
    drop(returned);
    calls.lock().unwrap().clear();
    statement.inner.lease_active.store(false, Ordering::Release);
    assert_eq!(
        rows.get_statement().map(|_| ()),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 0);
    connection.close().await.unwrap();
}

/// 空句柄短路不进入C或末端；精确错误反向退出且只记原语句一次。
#[tokio::test]
async fn task6_statement_filter_short_circuit_and_error_unwind_preserve_owner_counts() {
    use super::ordered_statement_filter::OrderedStatementFilter;
    use crate::core::{FilterChain, RdbcError};
    use std::sync::Mutex;
    for fail in [false, true] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        for name in ["A", "B", "C"] {
            filters.add_result_set(Arc::new(OrderedStatementFilter {
                name,
                calls: Arc::clone(&calls),
                short_circuit: name == "B",
                fail: fail && name == "B",
            }));
        }
        let (mut rows, mut statement, mut connection) =
            task6_sqlite_diagnostic_cursor_with_filters(Some(Arc::new(filters))).await;
        if fail {
            assert_eq!(
                rows.get_statement().map(|_| ()),
                Err(RdbcError::DriverError(
                    "ordered statement failure".to_owned()
                ))
            );
            assert_eq!(
                *calls.lock().unwrap(),
                ["A:before", "B:before", "B:error", "A:error"]
            );
            assert_eq!(statement.exception_count(), 1);
        } else {
            assert!(rows.get_statement().unwrap().is_none());
            assert_eq!(
                *calls.lock().unwrap(),
                ["A:before", "B:before", "B:ok", "A:ok"]
            );
            assert_eq!(statement.exception_count(), 0);
        }
        assert!(!statement.is_closed());
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
    let (mut rows, mut statement, mut connection) = task6_sqlite_diagnostic_cursor().await;
    rows.has_creating_statement = false;
    assert!(rows.get_statement().unwrap().is_none());
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
