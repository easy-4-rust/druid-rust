/// 活着的原生游标也不得接受旧逻辑租约的六种对象读取。
/// 对应 Java: DruidPooledResultSet#getObject 的逻辑连接生命周期检查。
#[tokio::test]
async fn task6_object_filter_expired_lease_rejects_before_hooks_and_native_reads() {
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
