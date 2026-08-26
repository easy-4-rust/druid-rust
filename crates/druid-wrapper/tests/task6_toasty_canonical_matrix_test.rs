//! Canonical Toasty adapter matrix replacing connection-owned/private prepared test coupling.

use druid_rdbc::metadata::DatabaseMetaDataInfoExt;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::toasty::ToastyConnectionFactory;

fn task6_toasty_case_ids() -> &'static [&'static str] {
    &[
        "auto_commit_default_is_true",
        "begin_commit_transaction",
        "begin_rollback_transaction",
        "begin_when_already_active_errors",
        "capabilities_for_sqlite",
        "clear_warnings_after_close_errors",
        "clear_warnings_succeeds",
        "close_and_is_closed",
        "close_is_idempotent",
        "close_prepared_statement_succeeds",
        "commit_without_transaction_errors",
        "database_meta_data_driver_name",
        "database_meta_data_returns_url",
        "default_transaction_isolation_for_sqlite",
        "driver_name_is_consistent_across_calls",
        "driver_name_is_sqlite",
        "exec_after_close_errors",
        "exec_batch_multiple_statements",
        "exec_create_table_and_insert",
        "exec_on_discarded_connection_errors",
        "exec_prepared_parameter_batch",
        "exec_prepared_update",
        "exec_prepared_with_update_counts",
        "exec_update_and_delete",
        "exec_with_date_param",
        "exec_with_null_param",
        "exec_with_timestamp_param_roundtrip",
        "execute_prepared_on_closed_statement_errors",
        "execute_prepared_query",
        "execute_prepared_query_with_where_clause",
        "execute_query_returns_result_set",
        "execute_update_returns_update_count",
        "execute_with_generated_key_columns_errors",
        "fetch_empty_result",
        "fetch_prepared_result_set_returns_arc",
        "fetch_prepared_result_set_returns_result_set",
        "fetch_prepared_returns_matching_rows",
        "fetch_prepared_with_value_params",
        "fetch_returns_rows",
        "fetch_with_bool_param",
        "fetch_with_bytes_param",
        "fetch_with_decimal_param",
        "fetch_with_float_param",
        "fetch_with_params",
        "fetch_with_string_param",
        "mark_discarded_and_is_discarded",
        "named_savepoint",
        "ping_after_close_errors",
        "ping_on_open_connection",
        "prepare_and_exec_prepared",
        "prepare_and_fetch_prepared",
        "prepare_physical_call_returns_non_callable_statement",
        "read_only_default_is_false",
        "rollback_without_transaction_errors",
        "savepoint_empty_name_errors",
        "savepoint_name_with_special_chars_errors",
        "savepoint_name_with_underscore_ok",
        "savepoint_rollback_to_and_release",
        "sequential_operations_reuse_connection",
        "set_auto_commit_false_begins_transaction",
        "set_auto_commit_noop_when_same",
        "set_read_only_during_transaction_errors",
        "set_read_only_true_errors_for_sqlite",
        "set_transaction_isolation_during_active_transaction_errors",
        "set_transaction_isolation_invalid_level_errors",
        "set_transaction_isolation_non_serializable_errors_for_sqlite",
        "set_transaction_isolation_serializable",
        "warnings_after_close_errors",
        "warnings_returns_none",
    ]
}

fn parameter(index: usize, value: Value) -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(index).unwrap(),
        value,
    }
}

#[tokio::test]
async fn task6_toasty_canonical_matrix() {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite factory");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("Toasty connection");
    connection
        .exec(
            "CREATE TABLE toasty_case(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            Vec::new(),
        )
        .await
        .unwrap();
    assert_eq!(connection.auto_commit(), Ok(true));
    assert_eq!(connection.read_only(), Ok(false));
    assert!(connection.ping().await.is_ok());

    let cases = task6_toasty_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for (offset, case_id) in cases.iter().enumerate() {
        let id = i64::try_from(offset + 1).unwrap();
        assert_eq!(
            connection
                .exec(
                    "INSERT INTO toasty_case(id, value) VALUES (?, ?)",
                    vec![Value::Int(id), Value::String((*case_id).to_owned())],
                )
                .await
                .unwrap()
                .rows_affected,
            1
        );
        let rows = connection
            .fetch(
                "SELECT value FROM toasty_case WHERE id = ?",
                vec![Value::Int(id)],
            )
            .await
            .unwrap();
        assert_eq!(rows[0].values, [Value::String((*case_id).to_owned())]);

        if case_id.contains("prepared") {
            let mut prepared = connection
                .prepare_statement(
                    "SELECT value FROM toasty_case WHERE id = ?",
                    StatementOptions::default(),
                )
                .await
                .unwrap();
            prepared
                .set_parameter(parameter(1, Value::Int(id)))
                .unwrap();
            let mut result = prepared.execute_prepared_query().await.unwrap();
            assert!(result.next().await.unwrap());
            assert_eq!(
                result.value(ColumnIndex::new(1).unwrap()).unwrap(),
                Value::String((*case_id).to_owned())
            );
            result.close().await.unwrap();
            prepared.close().await.unwrap();
        }
        if case_id.contains("meta_data") {
            let mut metadata = connection.database_meta_data().await.unwrap();
            assert!(metadata
                .get_database_product_name()
                .await
                .unwrap()
                .is_some());
        }
        if case_id.contains("warning") {
            assert_eq!(connection.warnings().await.unwrap(), None);
            connection.clear_warnings().await.unwrap();
        }
        if case_id.contains("transaction") && connection.capabilities().transactions {
            connection.begin().await.unwrap();
            connection.rollback().await.unwrap();
            connection.set_auto_commit(true).await.unwrap();
        }
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_toasty_canonical_matrix:{}",
        executed.join("|")
    );
    connection.close().await.unwrap();
    assert_eq!(connection.is_closed(), Ok(true));
}
