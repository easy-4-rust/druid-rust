//! 保留原完整标量getter案例；待当前全量回归终态接入。
#[path = "scalar_getter_delegation_probe.rs"]
mod scalar_getter_delegation_probe;
/// 原19调用和精确错误，不能以通用值getter代替。
/// 对应 Java: DruidPooledResultSet#getXxx。
#[tokio::test]
async fn task6_pooled_scalar_getters_delegate_exact_overloads_and_classify_errors() {
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::types::{RdbcObject, Value};
    let (mut statement, mut connection) = task6_sqlite_diagnostic_statement_with_filters(None).await;
    let failed_calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(scalar_getter_delegation_probe::ScalarGetterDelegationProbe::new(Arc::clone(&failed_calls), Some("boolean_by_label"))),
        &admission,
    ).await.unwrap();
    assert!(matches!(result_set.boolean_by_label("failing"), Err(RdbcError::DriverError(message)) if message == "boolean_by_label failed"));
    assert_eq!(statement.exception_count(), 1);
    assert_eq!(*failed_calls.lock().unwrap(), ["boolean_by_label"]);
    failed_calls.lock().unwrap().clear();
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut successful_result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(scalar_getter_delegation_probe::ScalarGetterDelegationProbe::new(Arc::clone(&calls), None)),
        &admission,
    ).await.unwrap();
    drop(admission);
    assert_eq!(successful_result_set.object_by_label("object").unwrap(), RdbcObject::Scalar(Value::Int(90)));
    assert_eq!(successful_result_set.string(ColumnIndex::new(1).unwrap()).unwrap().as_deref(), Some("index"));
    assert_eq!(successful_result_set.string_by_label("string").unwrap().as_deref(), Some("label"));
    assert!(successful_result_set.boolean(ColumnIndex::new(2).unwrap()).unwrap());
    assert!(!successful_result_set.boolean_by_label("boolean").unwrap());
    assert_eq!(successful_result_set.long(ColumnIndex::new(3).unwrap()).unwrap(), 101);
    assert_eq!(successful_result_set.long_by_label("long").unwrap(), 102);
    assert_eq!(successful_result_set.int(ColumnIndex::new(4).unwrap()).unwrap(), 103);
    assert_eq!(successful_result_set.int_by_label("int").unwrap(), 104);
    assert_eq!(successful_result_set.short(ColumnIndex::new(5).unwrap()).unwrap(), 105);
    assert_eq!(successful_result_set.short_by_label("short").unwrap(), 106);
    assert_eq!(successful_result_set.byte(ColumnIndex::new(6).unwrap()).unwrap(), 107);
    assert_eq!(successful_result_set.byte_by_label("byte").unwrap(), 108);
    assert_eq!(successful_result_set.double(ColumnIndex::new(7).unwrap()).unwrap(), 109.5);
    assert_eq!(successful_result_set.double_by_label("double").unwrap(), 110.5);
    assert_eq!(successful_result_set.float(ColumnIndex::new(8).unwrap()).unwrap(), 111.5);
    assert_eq!(successful_result_set.float_by_label("float").unwrap(), 112.5);
    assert_eq!(successful_result_set.bytes(ColumnIndex::new(9).unwrap()).unwrap(), Some(vec![113]));
    assert_eq!(successful_result_set.bytes_by_label("bytes").unwrap(), Some(vec![114]));
    assert_eq!(calls.lock().unwrap().len(), 19);
    assert_eq!(*calls.lock().unwrap(), ["value_by_label", "string", "string_by_label", "boolean", "boolean_by_label", "long", "long_by_label", "int", "int_by_label", "short", "short_by_label", "byte", "byte_by_label", "double", "double_by_label", "float", "float_by_label", "bytes", "bytes_by_label"]);
    assert_eq!(statement.exception_count(), 1);
    result_set.close().await.unwrap();
    successful_result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
