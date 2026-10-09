#[tokio::test]
async fn pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity() {
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ObjectShortCircuitFilter {
        calls: Arc::clone(&call_log),
    }));

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite 工厂必须创建成功");
    let physical = factory
        .create()
        .await
        .expect("必须创建真实 SQLite 物理连接");
    let mut connection = DruidPooledConnection::with_context(
        physical,
        75,
        "sqlite-object-filter".to_string(),
        Some(Arc::new(filter_chain)),
        Box::new(|_, _| {}),
    );
    let mut statement = connection.create_statement().await.unwrap();
    let mut result_set = statement
        .execute_query_result_set(&mut connection, "SELECT 1 AS value")
        .await
        .unwrap();

    let empty_map = RdbcTypeMap::new();
    let mut populated_map = RdbcTypeMap::new();
    populated_map.insert(
        "example.address",
        RdbcTargetType::Custom("example.Address".to_string()),
    );
    let custom_target = RdbcTargetType::Custom("example.Target".to_string());

    assert_eq!(
        result_set.object(&mut connection, 1).unwrap(),
        Value::Int(901)
    );
    assert_eq!(
        result_set
            .object_by_label(&mut connection, "value")
            .unwrap(),
        Value::String("filtered-label-object".to_string())
    );
    assert_eq!(
        result_set
            .object_with_type_map(&mut connection, 2, None)
            .unwrap(),
        RdbcObject::String("filtered-map:null".to_string())
    );
    assert_eq!(
        result_set
            .object_with_type_map(&mut connection, 3, Some(&empty_map))
            .unwrap(),
        RdbcObject::String("filtered-map:empty".to_string())
    );
    assert_eq!(
        result_set
            .object_by_label_with_type_map(&mut connection, "mapped", Some(&populated_map),)
            .unwrap(),
        RdbcObject::String(
            "filtered-label-map:example.address=Custom(\"example.Address\")".to_string()
        )
    );
    assert_eq!(
        result_set
            .object_typed(&mut connection, 4, &custom_target)
            .unwrap(),
        RdbcObject::String("filtered-typed:Custom(\"example.Target\")".to_string())
    );
    assert_eq!(
        result_set
            .object_typed_by_label(&mut connection, "typed", &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::String("filtered-label-typed:String".to_string())
    );

    assert_eq!(
        calls(&call_log),
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

    assert_eq!(
        result_set.object_typed_by_label(
            &mut connection,
            "fail",
            &RdbcTargetType::Custom("example.Fail".to_string()),
        ),
        Err(RdbcError::DriverError(
            "filtered object failure".to_string()
        ))
    );
    assert_eq!(statement.exception_count(), 1);
    result_set.close_with_connection(&mut connection).unwrap();
}
