/// 六种对象读取短路均服从隐藏列、精确错误计数和关闭边界。
/// 对应 Java: DruidPooledResultSet#getObject 的所有重载。
#[tokio::test]
async fn task6_object_filter_short_circuit_obeys_hidden_columns_and_closed_guards() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::types::RdbcTargetType;
    use object_short_circuit_filter::ObjectShortCircuitFilter;
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
