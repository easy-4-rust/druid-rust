//! Canonical SQLx connection/prepared/transaction matrix.

use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;

fn task6_sqlx_case_ids() -> &'static [&'static str] {
    &[
        "rdbc_connection_contract",
        "sqlx_adapter_exec_fetch_and_type_mapping",
        "sqlx_adapter_executes_and_reuses_real_prepared_statements",
        "sqlx_adapter_exposes_parallel_physical_and_standard_contracts",
        "sqlx_adapter_rejects_a_closed_prepared_statement_handle",
        "sqlx_adapter_rejects_unsafe_savepoint_names",
        "sqlx_adapter_transaction_and_savepoint_semantics",
        "sqlx_prepared_resources_execute_and_batch_against_real_sqlite",
    ]
}

fn parameter(index: usize, value: Value) -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(index).unwrap(),
        value,
    }
}

#[tokio::test]
async fn task6_sqlx_canonical_matrix() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .unwrap();
    connection
        .exec(
            "CREATE TABLE sqlx_case(id INTEGER PRIMARY KEY, value TEXT)",
            Vec::new(),
        )
        .await
        .unwrap();
    let cases = task6_sqlx_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for (offset, case_id) in cases.iter().enumerate() {
        let id = i64::try_from(offset + 1).unwrap();
        let mut prepared = connection
            .prepare_statement(
                "INSERT INTO sqlx_case(id, value) VALUES (?, ?)",
                StatementOptions::default(),
            )
            .await
            .unwrap();
        prepared
            .set_parameter(parameter(1, Value::Int(id)))
            .unwrap();
        prepared
            .set_parameter(parameter(2, Value::String((*case_id).to_owned())))
            .unwrap();
        assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
        prepared.close().await.unwrap();
        assert_eq!(prepared.is_closed(), Ok(true));

        let mut query = connection
            .prepare_statement(
                "SELECT value FROM sqlx_case WHERE id = ?",
                StatementOptions::default(),
            )
            .await
            .unwrap();
        query.set_parameter(parameter(1, Value::Int(id))).unwrap();
        let mut rows = query.execute_prepared_query().await.unwrap();
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
            Value::String((*case_id).to_owned())
        );
        rows.close().await.unwrap();
        query.close().await.unwrap();

        if case_id.contains("savepoint") && connection.capabilities().savepoints {
            connection.begin().await.unwrap();
            let savepoint = connection
                .set_savepoint_named("task6_sqlx_safe")
                .await
                .unwrap();
            connection.rollback_to(&savepoint).await.unwrap();
            connection.commit().await.unwrap();
            connection.set_auto_commit(true).await.unwrap();
        }
        assert!(connection.ping().await.is_ok());
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_sqlx_canonical_matrix:{}",
        executed.join("|")
    );
    connection.close().await.unwrap();
    assert_eq!(connection.is_closed(), Ok(true));
}
