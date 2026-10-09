/// 完整保留九类标量原重载、顺序、短路和精确错误计数；候选未运行。
#[tokio::test]
async fn task6_pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use scalar_short_circuit_filter::ScalarShortCircuitFilter;
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
