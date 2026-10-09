/// 九类原生默认委托及注册顺序/短路/错误回退；候选未运行。
#[tokio::test]
async fn task6_scalar_filter_preserves_all_native_overloads_order_short_circuit_and_error_unwind() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetRead;
    use druid_rdbc::types::{RdbcObject, Value};
    use ordered_scalar_filter::OrderedScalarFilter;
    use scalar_read_probe::{scalar_stem, ScalarReadProbe};
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

    // 两个真实默认Filter不改变原请求/原生返回；标签默认路径仍进入既有value hook列定位。
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
            "raw:find:virtual".to_owned(),
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
                "raw:find:virtual".to_owned(),
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
            "raw:find:fail",
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
