#[tokio::test]
async fn task6_pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity() {
    use crate::core::{FilterChain, RdbcError, Value};
    use druid_rdbc::types::{RdbcObject, RdbcTargetType, RdbcTypeMap};
    use object_short_circuit_filter::ObjectShortCircuitFilter;
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
