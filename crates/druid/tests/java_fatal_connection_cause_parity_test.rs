//! 对照 Java Oracle/OceanBase Oracle 致命错误后的连接原因链与回收。

mod standard_test_support;

use druid::core::{
    ExceptionSorter, MySqlExceptionSorter, OceanBaseOracleExceptionSorter, OracleExceptionSorter,
};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::statement::{GeneratedKeys, StatementOptions};
use standard_test_support::{TrackingFactory, TrackingState};
use std::error::Error as _;
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn assert_original_cause(error: &RdbcError, original: &RdbcError) {
    assert_eq!(error.class_name(), "druid::ConnectionDiscarded");
    assert_eq!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<RdbcError>()),
        Some(original),
        "typed original SQL error must remain the immediate source"
    );
    assert_eq!(error.sql_exception(), original.sql_exception());
}

#[tokio::test]
async fn oracle_and_oceanbase_fatal_errors_remain_sources_of_rejected_connection_calls() {
    let sorters: [(&str, Arc<dyn ExceptionSorter>); 2] = [
        ("oracle", Arc::new(OracleExceptionSorter::new())),
        (
            "oceanbase-oracle",
            Arc::new(OceanBaseOracleExceptionSorter::new()),
        ),
    ];
    for (name, sorter) in sorters {
        let state = TrackingState::configured();
        let pool = DruidPool::builder()
            .name(format!("java-fatal-cause-{name}"))
            .driver_name("tracking")
            .factory(Arc::new(TrackingFactory::new(Arc::clone(&state))))
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .build()
            .await
            .expect("pool");
        let mut first = pool.get().await.expect("warmup connection");
        let mut prepared = first
            .prepare_statement("SELECT 1", StatementOptions::default())
            .await
            .expect("warmup prepared");
        prepared.execute_prepared().await.expect("warmup execute");
        prepared.close().await.expect("warmup prepared close");
        first.close().await.expect("warmup logical close");

        let mut connection = pool
            .get()
            .await
            .expect("cached connection")
            .with_exception_sorter(sorter);
        let mut prepared = connection
            .prepare_statement("SELECT 1", StatementOptions::default())
            .await
            .expect("cached prepared");
        prepared.set_fetch_size(1_000).expect("fetch size");
        let original = RdbcError::SqlException(Box::new(SqlException::new(
            28,
            Some("xxx".to_owned()),
            Some("xx".to_owned()),
        )));
        assert!(connection.handle_rdbc_exception(&original));
        prepared
            .close()
            .await
            .expect("prepared cleanup after fatal error");
        let commits = pool.stats_collector().commit_count.load(Ordering::Acquire);
        assert_original_cause(&connection.commit().await.unwrap_err(), &original);
        assert_original_cause(&connection.auto_commit().unwrap_err(), &original);
        assert_eq!(
            pool.stats_collector().commit_count.load(Ordering::Acquire),
            commits
        );
        connection
            .close()
            .await
            .expect("discard logical connection");
        assert_eq!(pool.state().active_count, 0);
        assert_eq!(pool.state().discard_count, 1);
        let mut replacement = pool.get().await.expect("replacement has a fresh policy");
        assert_eq!(state.factory_create_count.load(Ordering::Acquire), 2);
        replacement
            .commit()
            .await
            .expect("replacement is unaffected by old fatal cause");
        replacement.close().await.expect("replacement close");
        pool.close().await;
    }
}

#[tokio::test]
async fn statement_policy_fatal_error_remains_source_of_rejected_commit() {
    let state = TrackingState::configured();
    let pool = DruidPool::builder()
        .name("java-statement-fatal-cause")
        .driver_name("tracking")
        .factory(Arc::new(TrackingFactory::new(Arc::clone(&state))))
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("pool");
    let mut connection = pool
        .get()
        .await
        .expect("connection")
        .with_exception_sorter(Arc::new(MySqlExceptionSorter));
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("statement");
    state.fail_next("statement_execute_update");
    let original = statement
        .execute_update("UPDATE t SET v=1", GeneratedKeys::None)
        .await
        .expect_err("fatal driver SQL error");
    assert_original_cause(&connection.commit().await.unwrap_err(), &original);
    statement.close().await.expect("statement cleanup");
    connection.close().await.expect("logical close");
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}
