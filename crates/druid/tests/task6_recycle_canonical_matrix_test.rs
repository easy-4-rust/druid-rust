//! Canonical Druid lease recycle/reset/discard matrix.

use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use std::sync::Arc;

fn task6_recycle_case_ids() -> &'static [&'static str] {
    &[
        "canonical_holder_moves_through_idle_queue_without_losing_java_metadata",
        "configured_isolation_preservation_skips_reset",
        "dirty_drop_discards_instead_of_leaking_transaction_to_idle_queue",
        "explicit_close_rolls_back_and_resets_in_java_order",
        "initialization_failure_closes_connection_and_releases_capacity",
        "mysql_family_db_type_enables_java_init_schema_restore_policy",
        "odps_skips_default_auto_commit_initialization",
        "physical_connection_defaults_are_initialized_in_java_order",
        "physical_connection_is_discarded_at_configured_max_use_count",
        "read_only_transaction_skips_rollback_then_restores_defaults",
        "rollback_failure_is_swallowed_counted_and_connection_discarded",
        "test_on_return_validates_and_invalid_connection_is_not_recycled",
    ]
}

#[tokio::test]
async fn task6_recycle_canonical_matrix() {
    let factory = Arc::new(SqlxConnectionFactory::new("sqlite::memory:"));
    let pool = DruidPool::builder()
        .name("task6-recycle-matrix")
        .driver_name("sqlx")
        .factory(factory)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut executed = Vec::new();
    for (offset, case_id) in task6_recycle_case_ids().iter().enumerate() {
        let mut lease = pool.get().await.unwrap();
        assert_eq!(pool.state().active_count, 1);
        assert!(lease.auto_commit().unwrap());
        if case_id.contains("dirty_drop") {
            lease.begin().await.unwrap();
            lease
                .exec(
                    "CREATE TABLE dirty_case(id INTEGER PRIMARY KEY)",
                    Vec::new(),
                )
                .await
                .unwrap();
            drop(lease);
        } else {
            lease
                .exec("SELECT ?", vec![Value::Int(i64::try_from(offset).unwrap())])
                .await
                .unwrap();
            lease.close().await.unwrap();
        }
        assert_eq!(pool.state().active_count, 0);
        let mut next = pool.get().await.unwrap();
        assert!(next.auto_commit().unwrap());
        next.close().await.unwrap();
        executed.push(*case_id);
    }
    assert_eq!(executed, task6_recycle_case_ids());
    println!(
        "TASK6_CASE_DIGEST=task6_recycle_canonical_matrix:{}",
        executed.join("|")
    );
    pool.close().await;
}
