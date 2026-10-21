//! 结果集诊断：filter_overloads_b 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 六种对象读取短路均服从隐藏列、精确错误计数和关闭边界。
/// 对应 Java: DruidPooledResultSet#getObject 的所有重载。
#[tokio::test]
async fn task6_object_filter_short_circuit_obeys_hidden_columns_and_closed_guards() {
    use super::object_short_circuit_filter::ObjectShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::types::RdbcTargetType;
    use std::sync::Mutex;

    let calls = Arc::new(Mutex::new(Vec::new()));
    let arguments = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(ObjectShortCircuitFilter {
        calls: Arc::clone(&calls),
        arguments: Arc::clone(&arguments),
    }));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT '原值' AS value",
            Some(Arc::new(filters)),
        )
        .await;
    let column = ColumnIndex::new(1).unwrap();
    let target = RdbcTargetType::Custom("example.Guarded".to_owned());

    // 所有重载使用同一真实列治理，拒绝发生在Filter执行之前。
    rows.set_hidden_columns(Some(vec![1]));
    let hidden_results = [
        rows.object(column),
        rows.object_by_label("value"),
        rows.object_with_type_map(column, None),
        rows.object_with_type_map_by_label("value", None),
        rows.object_as(column, &target),
        rows.object_as_by_label("value", &target),
    ];
    for result in hidden_results {
        assert_eq!(
            result,
            Err(RdbcError::InvalidArgument("column is hidden".to_owned()))
        );
    }
    assert!(calls.lock().unwrap().is_empty());
    assert!(arguments.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 6);

    rows.set_hidden_columns(None);
    assert_eq!(
        rows.object_as_by_label("fail", &target),
        Err(RdbcError::DriverError("filtered object failure".to_owned()))
    );
    assert_eq!(statement.exception_count(), 7);
    assert_eq!(
        *calls.lock().unwrap(),
        vec!["result_set_get_object_typed_by_label:fail:Custom(\"example.Guarded\")"]
    );
    assert_eq!(
        *arguments.lock().unwrap(),
        vec![std::ptr::from_ref(&target) as usize]
    );

    // 关闭后的虚拟标签不能通过短路复活游标，也不能额外记失败或调用Filter。
    rows.close().await.unwrap();
    let closed_results = [
        rows.object(column),
        rows.object_by_label("virtual"),
        rows.object_with_type_map(column, None),
        rows.object_with_type_map_by_label("virtual", None),
        rows.object_as(column, &target),
        rows.object_as_by_label("virtual", &target),
    ];
    for result in closed_results {
        assert_eq!(result, Err(RdbcError::ConnectionDiscarded));
    }
    assert_eq!(statement.exception_count(), 7);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert_eq!(arguments.lock().unwrap().len(), 1);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 默认对象Filter链保持六种原生入口、参数引用和资源返回身份。
#[tokio::test]
async fn task6_object_filter_default_preserves_resource_identity_and_native_overloads() {
    use super::object_payload::ObjectPayload;
    use super::object_read_probe::ObjectReadProbe;
    use crate::core::{FilterAdapter, FilterChain};
    use druid_rdbc::types::{
        RdbcObject, RdbcOpaqueObject, RdbcReader, RdbcTargetType, RdbcTypeMap,
    };
    use std::sync::Mutex;
    let stream = RdbcReader::from_string("资源对象😀");
    let payload = RdbcOpaqueObject::new(Arc::new(ObjectPayload("原对象".to_owned())));
    for object in [
        RdbcObject::CharacterStream(stream.clone()),
        RdbcObject::Custom(payload.clone()),
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let arguments = Arc::new(Mutex::new(Vec::new()));
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::new(FilterAdapter::new()));
        filters.add_result_set(Arc::new(FilterAdapter::new()));
        let (mut statement, mut connection) =
            task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
        let admission = statement.inner.begin_standard_admission().unwrap();
        let mut rows = DruidPooledResultSet::new_standard(
            Arc::clone(&statement.inner),
            Box::new(ObjectReadProbe {
                calls: Arc::clone(&calls),
                arguments: Arc::clone(&arguments),
                object: object.clone(),
                closed: false,
            }),
            &admission,
        )
        .await
        .unwrap();
        drop(admission);
        let map = RdbcTypeMap::new();
        let target = RdbcTargetType::Custom("example.Resource".to_owned());
        let index = ColumnIndex::new(1).unwrap();
        // 无行定位、无额外find_column、无目标类型预转换，原资源句柄无损返回。
        for returned in [
            rows.object(index),
            rows.object_by_label("ordinary"),
            rows.object_with_type_map(index, None),
            rows.object_with_type_map_by_label("mapped", Some(&map)),
            rows.object_as(index, &target),
            rows.object_as_by_label("typed", &target),
        ] {
            assert_eq!(returned.unwrap(), object);
        }
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "raw:ordinary:1",
                "raw:ordinary_label:ordinary",
                "raw:map:1:false",
                "raw:map_label:mapped:true",
                "raw:typed:1:Custom(\"example.Resource\")",
                "raw:typed_label:typed:Custom(\"example.Resource\")",
            ]
        );
        assert_eq!(
            *arguments.lock().unwrap(),
            vec![
                std::ptr::from_ref(&map) as usize,
                std::ptr::from_ref(&target) as usize,
                std::ptr::from_ref(&target) as usize,
            ]
        );
        assert_eq!(statement.exception_count(), 0);
        match rows.object_as_by_label("typed", &target).unwrap() {
            RdbcObject::CharacterStream(returned) => {
                let mut prefix = [0_u16; 2];
                assert_eq!(returned.read_utf16(&mut prefix).unwrap(), 2);
                assert_eq!(String::from_utf16(&prefix).unwrap(), "资源");
                assert_eq!(stream.read_to_string().unwrap(), "对象😀");
            }
            RdbcObject::Custom(returned) => {
                assert_eq!(returned, payload);
                assert!(std::ptr::eq(returned.value(), payload.value()));
                assert_eq!(
                    returned.downcast_ref::<ObjectPayload>().unwrap().0,
                    "原对象"
                );
            }
            other => panic!("完整对象不能转换为标量: {other:?}"),
        }
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
    }
}

/// 六入口共享注册顺序，错误按反向回退，资源短路不接触原驱动。
#[tokio::test]
async fn task6_object_filters_preserve_order_short_circuit_and_error_unwind() {
    use super::object_read_probe::ObjectReadProbe;
    use super::ordered_object_filter::OrderedObjectFilter;
    use crate::core::{FilterChain, RdbcError, Value};
    use druid_rdbc::types::{RdbcObject, RdbcReader, RdbcTargetType, RdbcTypeMap};
    use std::sync::Mutex;
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B"] {
        filters.add_result_set(Arc::new(OrderedObjectFilter {
            name,
            calls: Arc::clone(&log),
            short_circuit: None,
        }));
    }
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(ObjectReadProbe {
            calls: Arc::clone(&log),
            arguments: Arc::new(Mutex::new(Vec::new())),
            object: RdbcObject::Scalar(Value::Int(17)),
            closed: false,
        }),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let map = RdbcTypeMap::new();
    let target = RdbcTargetType::Custom("example.Order".to_owned());
    let column = ColumnIndex::new(1).unwrap();
    let returned = [
        rows.object(column),
        rows.object_by_label("ordinary"),
        rows.object_with_type_map(column, None),
        rows.object_with_type_map_by_label("mapped", Some(&map)),
        rows.object_as(column, &target),
        rows.object_as_by_label("typed", &target),
    ];
    for result in returned {
        assert_eq!(result.unwrap(), RdbcObject::Scalar(Value::Int(17)));
    }
    let terminals = [
        "raw:ordinary:1",
        "raw:ordinary_label:ordinary",
        "raw:map:1:false",
        "raw:map_label:mapped:true",
        "raw:typed:1:Custom(\"example.Order\")",
        "raw:typed_label:typed:Custom(\"example.Order\")",
    ];
    let expected: Vec<String> = terminals
        .into_iter()
        .flat_map(|terminal| ["A:before", "B:before", terminal, "B:ok", "A:ok"].map(str::to_owned))
        .collect();
    assert_eq!(*log.lock().unwrap(), expected);
    assert_eq!(statement.exception_count(), 0);
    log.lock().unwrap().clear();
    assert_eq!(
        rows.object_as_by_label("fail", &target),
        Err(RdbcError::DriverError("object probe failure".to_owned()))
    );
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "A:before",
            "B:before",
            "raw:typed_label:fail:Custom(\"example.Order\")",
            "B:error",
            "A:error"
        ]
    );
    assert_eq!(statement.exception_count(), 1);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();

    // 中间Filter原样返回资源，后续Filter和原生驱动均不得执行。
    let log = Arc::new(Mutex::new(Vec::new()));
    let stream = RdbcReader::from_string("短路资源😀");
    let object = RdbcObject::CharacterStream(stream.clone());
    let mut filters = FilterChain::new();
    for (name, short_circuit) in [("A", None), ("B", Some(object.clone())), ("C", None)] {
        filters.add_result_set(Arc::new(OrderedObjectFilter {
            name,
            calls: Arc::clone(&log),
            short_circuit,
        }));
    }
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT '原值' AS value",
            Some(Arc::new(filters)),
        )
        .await;
    let virtual_column = ColumnIndex::new(3).unwrap();
    for result in [
        rows.object(virtual_column),
        rows.object_by_label("virtual"),
        rows.object_with_type_map(virtual_column, None),
        rows.object_with_type_map_by_label("virtual", Some(&map)),
        rows.object_as(virtual_column, &target),
        rows.object_as_by_label("virtual", &target),
    ] {
        assert_eq!(result.unwrap(), object);
    }
    assert_eq!(
        *log.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
            .into_iter()
            .cycle()
            .take(24)
            .map(str::to_owned)
            .collect::<Vec<_>>()
    );
    assert_eq!(stream.read_to_string().unwrap(), "短路资源😀");
    assert_eq!(statement.exception_count(), 0);
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 活着的原生游标也不得接受旧逻辑租约的六种对象读取。
/// 对应 Java: DruidPooledResultSet#getObject 的逻辑连接生命周期检查。
#[tokio::test]
async fn task6_object_filter_expired_lease_rejects_before_hooks_and_native_reads() {
    use super::object_short_circuit_filter::ObjectShortCircuitFilter;
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::types::RdbcTargetType;
    use std::sync::Mutex;
    let calls = Arc::new(Mutex::new(Vec::new()));
    let arguments = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(ObjectShortCircuitFilter {
        calls: Arc::clone(&calls),
        arguments: Arc::clone(&arguments),
    }));
    let (mut rows, mut statement, mut connection) =
        task6_sqlite_diagnostic_cursor_with_sql_and_filters(
            "SELECT '原值' AS value",
            Some(Arc::new(filters)),
        )
        .await;
    assert!(!rows.is_closed().unwrap());
    assert!(!rows
        .resource
        .result_set
        .lock()
        .await
        .as_ref()
        .unwrap()
        .is_closed()
        .unwrap());
    statement.inner.lease_active.store(false, Ordering::Release);
    let column = ColumnIndex::new(1).unwrap();
    let target = RdbcTargetType::Custom("example.Expired".to_owned());
    for result in [
        rows.object(column),
        rows.object_by_label("virtual"),
        rows.object_with_type_map(column, None),
        rows.object_with_type_map_by_label("virtual", None),
        rows.object_as(column, &target),
        rows.object_as_by_label("virtual", &target),
    ] {
        assert_eq!(result, Err(RdbcError::ConnectionDiscarded));
    }
    assert!(calls.lock().unwrap().is_empty());
    assert!(arguments.lock().unwrap().is_empty());
    assert_eq!(statement.exception_count(), 0);
    // 原游标没有被读取失败误关闭；后续显式清理不能恢复旧租约。
    assert!(!rows
        .resource
        .result_set
        .lock()
        .await
        .as_ref()
        .unwrap()
        .is_closed()
        .unwrap());
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
