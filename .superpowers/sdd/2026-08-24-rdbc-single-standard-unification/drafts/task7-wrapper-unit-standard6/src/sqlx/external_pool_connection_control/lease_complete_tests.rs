use super::{take_poisoned, ExternalPoolLeaseControl};
use druid::core::ExternalConnectionLease;
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::extensions::ConnectionCapabilities;
use druid_rdbc::metadata::DatabaseMetaDataInfoExt;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::result_set::ResultSetExt;
use druid_rdbc::statement::{
    CallableStatement, ExecutionResult, GeneratedKeys, ParameterIndex, PreparedParameter,
    PreparedStatement, ResultSetHoldability, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
mod delegation_counts;
mod delegation_resources;
mod lease_blob_access;
mod lease_clob_access;
mod lease_contract_connection;
mod lease_contract_lease;
mod lease_meta_data;
mod lease_statement_probe;
use delegation_counts::DelegationCounts;
use delegation_resources::DelegationResources;
use lease_contract_connection::ContractConnection;
use lease_contract_lease::ContractLease;
fn delegation_resources(counts: Arc<DelegationCounts>) -> DelegationResources {
    DelegationResources::new(counts)
}
fn connection_lease_with_delegation(
    drop_count: Arc<AtomicUsize>,
    counts: Arc<DelegationCounts>,
    resources: DelegationResources,
) -> ExternalConnectionLease<ContractLease> {
    ExternalConnectionLease::new(ContractLease {
        connection: Box::new(ContractConnection {
            closed: false,
            auto_commit: true,
            read_only: false,
            isolation: TransactionIsolation::READ_COMMITTED,
            holdability: ResultSetHoldability::DriverValue(0),
            catalog: "catalog_a".into(),
            schema: "schema_a".into(),
            counts,
            resources,
        }),
        drop_count,
    })
}
fn statement_data_address(value: &dyn Statement) -> usize {
    std::ptr::from_ref::<dyn Statement>(value).cast::<()>() as usize
}
fn prepared_data_address(value: &dyn PreparedStatement) -> usize {
    std::ptr::from_ref::<dyn PreparedStatement>(value).cast::<()>() as usize
}
fn callable_data_address(value: &dyn CallableStatement) -> usize {
    std::ptr::from_ref::<dyn CallableStatement>(value).cast::<()>() as usize
}
fn bind_values(statement: &mut dyn PreparedStatement, values: Vec<Value>) {
    statement
        .clear_parameters()
        .expect("clear original parameter snapshot");
    for (offset, value) in values.into_iter().enumerate() {
        statement
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(offset + 1).unwrap(),
                value,
            })
            .expect("actual scalar binding");
    }
}
fn row_values(result_set: &dyn druid_rdbc::result_set::ResultSet) -> Vec<Value> {
    let metadata = result_set.meta_data().expect("actual row width");
    (1..=metadata.column_count())
        .map(|column| {
            result_set
                .value(ColumnIndex::new(column).unwrap())
                .expect("original complete row value")
        })
        .collect()
}
fn update_count(result: &ExecutionResult) -> i64 {
    match result {
        ExecutionResult::UpdateCount(count) => *count,
        ExecutionResult::ResultSet(_) => panic!("original expected update result"),
    }
}
// 完整固定 BASE 案例的标准 API 测试草稿；fixture/helper 尚未接入，不计迁移完成。
#[tokio::test]
async fn task6_lease_delegates_complete_spi() {
    let drop_count = Arc::new(AtomicUsize::new(0));
    let delegation_counts = Arc::new(DelegationCounts::default());
    let delegation_resources = delegation_resources(Arc::clone(&delegation_counts));
    let statement_address = delegation_resources.statement_address();
    let prepared_address = delegation_resources.prepared_address();
    let callable_address = delegation_resources.callable_address();
    let expected_blob = std::ptr::from_ref::<dyn druid_rdbc::resource::Blob>(
        delegation_resources.blob.as_deref().unwrap(),
    )
    .cast::<()>() as usize;
    let expected_clob = std::ptr::from_ref::<dyn druid_rdbc::resource::Clob>(
        delegation_resources.clob.as_deref().unwrap(),
    )
    .cast::<()>() as usize;
    let expected_n_clob = std::ptr::from_ref::<dyn druid_rdbc::resource::NClob>(
        delegation_resources.n_clob.as_deref().unwrap(),
    )
    .cast::<()>() as usize;
    let mut connection = connection_lease_with_delegation(
        drop_count.clone(),
        Arc::clone(&delegation_counts),
        delegation_resources,
    );

    let statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("physical statement must delegate");
    assert_eq!(
        statement_data_address(statement.as_ref()),
        statement_address
    );
    let mut delegated_prepared = connection
        .prepare_statement("UPDATE delegated prepared", StatementOptions::default())
        .await
        .expect("physical prepare must delegate");
    assert_eq!(
        prepared_data_address(delegated_prepared.as_ref()),
        prepared_address
    );
    let callable = connection
        .prepare_call("CALL delegated", StatementOptions::default())
        .await
        .expect("physical call must delegate");
    assert_eq!(callable_data_address(callable.as_ref()), callable_address);
    assert_eq!(
        connection
            .exec_batch(vec![(
                "UPDATE delegated batch".to_string(),
                vec![Value::Int(11)]
            )])
            .await
            .expect("batch must delegate"),
        vec![11]
    );
    bind_values(delegated_prepared.as_mut(), vec![Value::Int(12)]);
    assert_eq!(
        delegated_prepared
            .execute_prepared_update()
            .await
            .expect("prepared exec must delegate"),
        12
    );
    bind_values(delegated_prepared.as_mut(), vec![Value::Int(13)]);
    let mut delegated_rows = delegated_prepared
        .execute_prepared_query()
        .await
        .expect("prepared fetch must delegate");
    assert!(delegated_rows.next().await.expect("delegated row"));
    assert_eq!(row_values(delegated_rows.as_ref()), vec![Value::Int(13)]);
    let mut metadata = connection
        .database_meta_data()
        .await
        .expect("metadata must delegate");
    assert_eq!(
        metadata
            .get_database_product_name()
            .await
            .expect("metadata value must delegate"),
        Some("lease-metadata".to_string())
    );
    drop(metadata);
    let mut actual_blob = connection.create_blob().await.expect("blob must delegate");
    assert_eq!(
        std::ptr::from_ref::<dyn druid_rdbc::resource::Blob>(actual_blob.as_ref()).cast::<()>()
            as usize,
        expected_blob
    );
    assert_eq!(actual_blob.length().await.unwrap(), 4);
    let mut actual_clob = connection.create_clob().await.expect("clob must delegate");
    assert_eq!(
        std::ptr::from_ref::<dyn druid_rdbc::resource::Clob>(actual_clob.as_ref()).cast::<()>()
            as usize,
        expected_clob
    );
    assert_eq!(actual_clob.length().await.unwrap(), 4);
    let mut actual_n_clob = connection
        .create_n_clob()
        .await
        .expect("nclob must delegate");
    assert_eq!(
        std::ptr::from_ref::<dyn druid_rdbc::resource::NClob>(actual_n_clob.as_ref()).cast::<()>()
            as usize,
        expected_n_clob
    );
    assert_eq!(actual_n_clob.length().await.unwrap(), 4);
    assert_eq!(
        delegation_counts
            .create_physical_statement
            .load(Ordering::Relaxed),
        1
    );
    assert_eq!(
        delegation_counts
            .prepare_physical_statement
            .load(Ordering::Relaxed),
        1
    );
    assert_eq!(
        delegation_counts
            .prepare_physical_call
            .load(Ordering::Relaxed),
        1
    );
    assert_eq!(delegation_counts.exec_batch.load(Ordering::Relaxed), 1);
    assert_eq!(delegation_counts.exec_prepared.load(Ordering::Relaxed), 1);
    assert_eq!(delegation_counts.fetch_prepared.load(Ordering::Relaxed), 1);
    assert_eq!(
        delegation_counts.database_meta_data.load(Ordering::Relaxed),
        1
    );
    assert_eq!(delegation_counts.create_blob.load(Ordering::Relaxed), 1);
    assert_eq!(delegation_counts.create_clob.load(Ordering::Relaxed), 1);
    assert_eq!(delegation_counts.create_n_clob.load(Ordering::Relaxed), 1);

    let exec_result = connection
        .exec("UPDATE contract", vec![Value::Int(7)])
        .await
        .expect("exec must delegate");
    assert_eq!(exec_result.rows_affected, 1);
    let mut generic_statement = connection
        .prepare_statement_with_generated_keys(
            "EXECUTE contract",
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .expect("generic statement preparation");
    bind_values(generic_statement.as_mut(), vec![Value::Int(8)]);
    let execute_results = Statement::execute(
        generic_statement.as_mut(),
        "EXECUTE contract",
        GeneratedKeys::ReturnGeneratedKeys,
    )
    .await
    .expect("generic execute must delegate");
    assert_eq!(update_count(&execute_results[0]), 3);
    assert!(!matches!(
        &execute_results[0],
        ExecutionResult::ResultSet(_)
    ));
    let rows = connection
        .fetch("SELECT contract", vec![Value::String("value".to_string())])
        .await
        .expect("fetch must delegate");
    assert_eq!(rows[0].values, vec![Value::Bool(true)]);
    // 原带参数 fetch_result_set 在唯一标准树映射为 owned prepared query。
    let mut query_statement = connection
        .prepare_statement("SELECT contract", StatementOptions::default())
        .await
        .expect("query preparation");
    bind_values(
        query_statement.as_mut(),
        vec![Value::String("value".to_string())],
    );
    let mut result_set = query_statement
        .execute_query("SELECT contract")
        .await
        .expect("ResultSet fetch must delegate");
    assert!(result_set.next().await.unwrap());
    assert!(result_set.boolean(ColumnIndex::new(1).unwrap()).unwrap());
    let mut query_prepared = connection
        .prepare_statement("SELECT contract", StatementOptions::default())
        .await
        .expect("prepared query");
    bind_values(
        query_prepared.as_mut(),
        vec![Value::String("value".to_string())],
    );
    let mut prepared_result_set = query_prepared
        .execute_prepared_query()
        .await
        .expect("prepared ResultSet fetch must delegate");
    assert!(prepared_result_set.next().await.unwrap());
    assert!(prepared_result_set
        .boolean(ColumnIndex::new(1).unwrap())
        .unwrap());
    let mut generic_prepared = connection
        .prepare_statement_with_generated_keys(
            "EXECUTE contract",
            StatementOptions::default(),
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .expect("prepared generic");
    bind_values(generic_prepared.as_mut(), vec![Value::Int(8)]);
    let prepared_results = generic_prepared
        .execute_prepared()
        .await
        .expect("prepared generic execute must delegate");
    assert_eq!(update_count(&prepared_results[0]), 3);
    let mut prepared = connection
        .prepare_statement("UPDATE contract", StatementOptions::default())
        .await
        .expect("prepared batch");
    assert_eq!(
        connection
            .exec_prepared_batch(prepared.as_mut(), vec![vec![Value::Int(7)]])
            .await
            .expect("prepared batch must delegate"),
        [1]
    );
    prepared
        .close()
        .await
        .expect("prepared close must delegate");
    assert!(prepared.is_closed().unwrap());

    connection.begin().await.expect("begin must delegate");
    assert!(!connection.auto_commit().unwrap());
    connection.rollback().await.expect("rollback must delegate");
    connection
        .begin()
        .await
        .expect("second begin must delegate");
    connection.commit().await.expect("commit must delegate");
    connection
        .set_auto_commit(false)
        .await
        .expect("set auto commit must delegate");
    connection
        .set_read_only(true)
        .await
        .expect("set read only must delegate");
    connection
        .set_transaction_isolation(TransactionIsolation::SERIALIZABLE)
        .await
        .expect("set isolation must delegate");
    connection
        .set_holdability(ResultSetHoldability::HoldOverCommit)
        .await
        .expect("set holdability must delegate");
    assert_eq!(
        connection
            .warnings()
            .await
            .expect("get warnings must delegate")
            .expect("warning must exist")
            .message(),
        "lease-warning"
    );
    connection
        .clear_warnings()
        .await
        .expect("clear warnings must delegate");
    connection
        .set_catalog(Some("catalog_b"))
        .await
        .expect("set catalog must delegate");
    connection
        .set_schema(Some("schema_b"))
        .await
        .expect("set schema must delegate");
    assert!(!connection.auto_commit().unwrap());
    assert!(connection.read_only().unwrap());
    assert_eq!(connection.transaction_isolation().unwrap().jdbc_code(), 8);
    assert_eq!(connection.holdability().unwrap().rdbc_code(), 1);
    assert_eq!(connection.catalog().unwrap().as_deref(), Some("catalog_b"));
    assert_eq!(connection.schema().unwrap().as_deref(), Some("schema_b"));

    let anonymous = connection
        .set_savepoint()
        .await
        .expect("anonymous savepoint must delegate");
    connection
        .rollback_to(&anonymous)
        .await
        .expect("rollback to savepoint must delegate");
    let named = connection
        .set_savepoint_named("named")
        .await
        .expect("named savepoint must delegate");
    connection
        .release_savepoint(&named)
        .await
        .expect("release savepoint must delegate");

    connection.ping().await.expect("ping must delegate");
    assert_eq!(connection.driver_name().unwrap(), "lease-contract");
    assert_eq!(
        connection.capabilities(),
        ConnectionCapabilities {
            transactions: true,
            savepoints: true,
            catalogs: true,
            schemas: true,
            callable_statements: true
        }
    );
    let manager_rejections = Arc::new(AtomicU64::new(0));
    let poison_control =
        ExternalPoolLeaseControl::register(&connection, Arc::clone(&manager_rejections));
    assert!(!take_poisoned(&connection));
    poison_control.poison();
    assert!(take_poisoned(&connection));
    assert_eq!(manager_rejections.load(Ordering::Relaxed), 1);
    connection.abort().await.expect("abort must delegate");
    assert!(connection.is_closed().unwrap());
    connection.close().await.expect("close must delegate");

    drop(connection);
    assert_eq!(drop_count.load(Ordering::Relaxed), 1);
}
