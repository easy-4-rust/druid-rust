/// 九类标量短路仍须服从全部下标/标签隐藏列、关闭及失效租约边界；候选未运行。
#[tokio::test]
async fn task6_scalar_filter_all_overloads_obey_hidden_closed_and_expired_lease_guards() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::{ResultSetExt, ResultSetRead};
    use scalar_short_circuit_filter::ScalarShortCircuitFilter;
    use std::collections::HashSet;
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
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let mut rows = statement.execute_query("SELECT 1 AS value").await.unwrap();
    rows.set_hidden_columns(Some(HashSet::from([1])));
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
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(Some(Arc::new(filters))).await;
    let mut rows = statement.execute_query("SELECT 1 AS value").await.unwrap();
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
