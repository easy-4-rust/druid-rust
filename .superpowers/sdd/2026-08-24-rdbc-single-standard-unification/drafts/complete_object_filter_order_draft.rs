/// 六入口共享注册顺序，错误按反向回退，资源短路不接触原驱动。
#[tokio::test]
async fn task6_object_filters_preserve_order_short_circuit_and_error_unwind() {
    use crate::core::{FilterChain, RdbcError, Value};
    use druid_rdbc::types::{RdbcObject, RdbcReader, RdbcTargetType, RdbcTypeMap};
    use object_read_probe::ObjectReadProbe;
    use ordered_object_filter::OrderedObjectFilter;
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
