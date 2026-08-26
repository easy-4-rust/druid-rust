//! Canonical owned-PreparedStatement matrix replacing private SqlText/cache test coupling.

use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    ParameterIndex, PreparedParameter, PreparedStatement, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;

fn task6_prepared_statement_case_ids() -> &'static [&'static str] {
    &[
        "clear_batch_failure_uses_connection_sorter_before_cache_return",
        "clear_parameters_failure_uses_connection_sorter_before_cache_return",
        "concurrent_logical_statements_preserve_java_in_use_and_lru_semantics",
        "direct_prepared_setter_and_batch_methods_route_failures_through_connection_sorter",
        "holder_statement_pool_is_lazy_and_debug_does_not_initialize_it",
        "mysql_schema_change_clears_existing_statement_cache",
        "pooled_prepared_statement_forwards_statement_properties_and_restores_real_sqlite_handle",
        "prepared_batch_snapshots_parameters_and_executes_against_real_sqlite",
        "prepared_bound_setters_execute_and_batch_against_real_sqlite",
        "prepared_close_restores_properties_in_java_order_and_stops_on_first_error",
        "prepared_generic_execute_matches_real_sqlite_result_and_generated_key_semantics",
        "prepared_generic_execute_preserves_ordered_multi_results",
        "prepared_input_parameter_converts_every_scalar_family_without_losing_type",
        "prepared_result_getters_route_driver_failures_through_connection_sorter",
        "prepared_result_set_keeps_the_same_statement_alive_and_can_close_it",
        "prepared_setter_overloads_preserve_exact_descriptor_identity",
        "prepared_statement_cannot_cross_connection_lease_boundary",
        "prepared_statement_reuses_physical_handle_and_updates_java_stats",
        "prepared_stream_reader_lob_url_rowid_and_null_bind_against_real_sqlite",
        "result_set_statement_handle_reports_clear_batch_failure",
        "result_set_statement_handle_reports_clear_parameters_failure",
        "statement_execution_error_removes_handle_instead_of_reusing_it",
    ]
}

fn parameter(index: usize, value: Value) -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(index).expect("1-based parameter"),
        value,
    }
}

#[tokio::test]
async fn task6_prepared_statement_canonical_matrix() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("SQLx connection");
    connection
        .exec(
            "CREATE TABLE prepared_case(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            Vec::new(),
        )
        .await
        .expect("table");
    let cases = task6_prepared_statement_case_ids();
    let mut executed = Vec::with_capacity(cases.len());

    for (offset, case_id) in cases.iter().enumerate() {
        let id = i64::try_from(offset + 1).unwrap();
        let sql = "INSERT INTO prepared_case(id, value) VALUES (?, ?)";
        let mut prepared = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .expect("prepared insert");
        assert_eq!(PreparedStatement::sql(&*prepared), Ok(sql));
        prepared
            .set_parameter(parameter(1, Value::Int(id)))
            .unwrap();
        prepared
            .set_parameter(parameter(2, Value::String((*case_id).to_owned())))
            .unwrap();
        assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
        prepared.clear_parameters().unwrap();
        prepared.clear_prepared_batch().unwrap();
        prepared.set_query_timeout(1).unwrap();
        assert_eq!(prepared.query_timeout(), Ok(1));

        if case_id.contains("batch") {
            for extra in 1_i64..=2 {
                prepared
                    .add_parameter_batch(vec![
                        parameter(1, Value::Int(1_000 + id * 10 + extra)),
                        parameter(2, Value::String(format!("{case_id}-{extra}"))),
                    ])
                    .unwrap();
            }
            assert_eq!(prepared.execute_prepared_batch().await.unwrap(), [1, 1]);
        }
        if case_id.contains("result") || case_id.contains("generic") {
            let mut query = connection
                .prepare_statement(
                    "SELECT value FROM prepared_case WHERE id = ?",
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
        }
        prepared.close().await.unwrap();
        assert_eq!(prepared.is_closed(), Ok(true));
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_prepared_statement_canonical_matrix:{}",
        executed.join("|")
    );
    connection.close().await.unwrap();
}
