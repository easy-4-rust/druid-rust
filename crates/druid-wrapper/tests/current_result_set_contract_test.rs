//! 验证当前结果 getter 保留游标身份，附属生成键不覆盖主结果序列。

#[path = "../../druid/tests/standard_test_support/mod.rs"]
mod standard_test_support;

use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::Arc;

#[tokio::test]
async fn current_result_getter_shares_real_sqlite_cursor_and_close_state() {
    for prepared in [false, true] {
        let pool = DruidPool::builder()
            .factory(Arc::new(SqlxConnectionFactory::new("sqlite::memory:"))
                as Arc<dyn ConnectionFactory>)
            .max_open(1)
            .max_idle(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let sql = "SELECT 7 AS value UNION ALL SELECT 8";
        let (mut statement, mut original): (Box<dyn Statement>, _) = if prepared {
            let mut statement = connection
                .prepare_statement(sql, StatementOptions::default())
                .await
                .unwrap();
            let rows = statement.execute_prepared_query().await.unwrap();
            (statement, rows)
        } else {
            let mut statement = connection
                .create_statement(StatementOptions::default())
                .await
                .unwrap();
            let rows = statement.execute_query(sql).await.unwrap();
            (statement, rows)
        };
        let column = ColumnIndex::new(1).unwrap();
        let mut alias = statement.get_result_set().await.unwrap().unwrap();
        assert!(original.next().await.unwrap());
        assert_eq!(alias.value(column).unwrap(), Value::Int(7));
        assert!(alias.next().await.unwrap());
        assert_eq!(original.value(column).unwrap(), Value::Int(8));
        let mut repeated = statement.get_result_set().await.unwrap().unwrap();
        assert_eq!(repeated.value(column).unwrap(), Value::Int(8));
        // 原始返回值释放后，registry 仍保留当前游标；getter 不复制数据或重置位置。
        drop(original);
        drop(alias);
        let mut surviving = statement.get_result_set().await.unwrap().unwrap();
        assert_eq!(surviving.value(column).unwrap(), Value::Int(8));
        surviving.close().await.unwrap();
        assert!(repeated.is_closed().unwrap());
        assert!(repeated.next().await.is_err());
        repeated.close().await.unwrap();
        statement.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
    }
}

#[tokio::test]
async fn callable_current_result_and_generated_keys_preserve_ordered_sequence() {
    let state = TrackingState::configured();
    let pool = DruidPool::builder()
        .factory(Arc::new(TrackingFactory::new(state)) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .prepare_call("MULTI", StatementOptions::default())
        .await
        .unwrap();
    let results = statement.execute_prepared().await.unwrap();
    assert!(matches!(
        results.first(),
        Some(ExecutionResult::ResultSet(_))
    ));
    let column = ColumnIndex::new(1).unwrap();
    let mut first = statement.get_result_set().await.unwrap().unwrap();
    assert!(first.next().await.unwrap());
    assert_eq!(first.value(column).unwrap(), Value::Int(10));
    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    assert!(first.is_closed().unwrap());
    assert_eq!(statement.get_update_count().unwrap(), Some(2));
    assert!(statement.get_result_set().await.unwrap().is_none());
    let mut keys = statement.generated_keys().await.unwrap();
    assert!(keys.next().await.unwrap());
    assert_eq!(keys.value(column).unwrap(), Value::Int(99));
    keys.close().await.unwrap();
    assert_eq!(statement.get_update_count().unwrap(), Some(2));
    assert!(statement
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    let mut third = statement.get_result_set().await.unwrap().unwrap();
    assert!(third.next().await.unwrap());
    assert_eq!(third.value(column).unwrap(), Value::Int(20));
    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    assert!(third.is_closed().unwrap());
    assert_eq!(statement.get_update_count().unwrap(), None);
    assert!(statement.get_result_set().await.unwrap().is_none());
    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    assert_eq!(statement.get_update_count().unwrap(), None);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[path = "support/current_result_handoff/mod.rs"]
mod current_result_handoff;

#[tokio::test]
async fn current_result_adopts_owned_driver_view_without_reopening_or_losing_mixed_results() {
    use current_result_handoff::{DriverState, HandoffFactory, OpenFilter};
    use druid::core::FilterChain;
    use druid_rdbc::statement::GeneratedKeys;
    use std::sync::atomic::Ordering;
    for kind in ["ordinary", "prepared", "callable"] {
        let state = Arc::new(DriverState::new());
        let mut filters = FilterChain::new();
        filters.add_result_set(Arc::new(OpenFilter(Arc::clone(&state))));
        let pool = DruidPool::builder()
            .factory(Arc::new(HandoffFactory(Arc::clone(&state))))
            .filter_chain(Arc::new(filters))
            .max_open(1)
            .max_idle(1)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let (mut statement, mut results): (Box<dyn Statement>, Vec<ExecutionResult>) = match kind {
            "ordinary" => {
                let mut s = connection
                    .create_statement(StatementOptions::default())
                    .await
                    .unwrap();
                let results = s.execute("MULTI", GeneratedKeys::None).await.unwrap();
                (s, results)
            }
            "prepared" => {
                let mut s = connection
                    .prepare_statement("MULTI", StatementOptions::default())
                    .await
                    .unwrap();
                let results = s.execute_prepared().await.unwrap();
                (s, results)
            }
            _ => {
                let mut s = connection
                    .prepare_call("MULTI", StatementOptions::default())
                    .await
                    .unwrap();
                let results = s.execute_prepared().await.unwrap();
                (s, results)
            }
        };
        let mut original = match results.remove(0) {
            ExecutionResult::ResultSet(rows) => rows,
            _ => panic!("first cursor"),
        };
        let column = ColumnIndex::new(1).unwrap();
        assert_eq!(state.open_ids.lock().unwrap().len(), 2);
        let mut alias = statement.get_result_set().await.unwrap().unwrap();
        assert_eq!(
            state.getter_calls.load(Ordering::Acquire),
            1,
            "{kind} must call the raw getter"
        );
        assert!(original.next().await.unwrap());
        assert_eq!(
            state.first.getter_reads.load(Ordering::Acquire),
            1,
            "must adopt, not discard the owned raw view"
        );
        assert_eq!(state.first.original_reads.load(Ordering::Acquire), 0);
        assert_eq!(state.first.original_drops.load(Ordering::Acquire), 1);
        assert_eq!(alias.value(column).unwrap(), Value::Int(7));
        let mut repeated = statement.get_result_set().await.unwrap().unwrap();
        assert_eq!(state.getter_calls.load(Ordering::Acquire), 2);
        assert_eq!(state.first.getter_drops.load(Ordering::Acquire), 1);
        assert!(alias.next().await.unwrap());
        assert_eq!(original.value(column).unwrap(), Value::Int(8));
        assert_eq!(repeated.value(column).unwrap(), Value::Int(8));
        assert_eq!(state.open_ids.lock().unwrap().len(), 2);
        let ids = state.read_ids.lock().unwrap().clone();
        assert_eq!(ids, [state.open_ids.lock().unwrap()[0]; 2]);
        // driver getter 返回已关闭视图时不能复活共享游标或再次释放 driver。
        original.close().await.unwrap();
        let closed_alias = statement.get_result_set().await.unwrap().unwrap();
        assert!(closed_alias.is_closed().unwrap());
        assert_eq!(state.first.close_calls.load(Ordering::Acquire), 1);
        assert_eq!(state.open_ids.lock().unwrap().len(), 2);
        assert!(!statement
            .get_more_results(CurrentResultDisposition::CloseCurrent)
            .await
            .unwrap());
        assert!(original.is_closed().unwrap());
        assert!(alias.is_closed().unwrap());
        assert!(repeated.is_closed().unwrap());
        assert_eq!(state.first.close_calls.load(Ordering::Acquire), 1);
        assert_eq!(statement.get_update_count().unwrap(), Some(2));
        assert!(statement.get_result_set().await.unwrap().is_none());
        assert!(statement
            .get_more_results(CurrentResultDisposition::CloseCurrent)
            .await
            .unwrap());
        let mut third = statement.get_result_set().await.unwrap().unwrap();
        assert!(third.next().await.unwrap());
        assert_eq!(third.value(column).unwrap(), Value::Int(20));
        assert_eq!(state.third.getter_reads.load(Ordering::Acquire), 1);
        assert_eq!(state.third.original_drops.load(Ordering::Acquire), 1);
        assert_eq!(state.open_ids.lock().unwrap().len(), 2);
        assert!(!statement
            .get_more_results(CurrentResultDisposition::CloseCurrent)
            .await
            .unwrap());
        assert_eq!(statement.get_update_count().unwrap(), None);
        assert!(statement.get_result_set().await.unwrap().is_none());
        assert!(!statement
            .get_more_results(CurrentResultDisposition::CloseCurrent)
            .await
            .unwrap());
        original.close().await.unwrap();
        alias.close().await.unwrap();
        repeated.close().await.unwrap();
        third.close().await.unwrap();
        assert_eq!(state.first.close_calls.load(Ordering::Acquire), 1);
        assert_eq!(state.third.close_calls.load(Ordering::Acquire), 1);
        drop(results);
        statement.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
        assert_eq!(state.first.close_calls.load(Ordering::Acquire), 1);
        assert_eq!(state.third.close_calls.load(Ordering::Acquire), 1);
        assert!(state.statement_closed.load(Ordering::Acquire));
        assert!(state.connection_closed.load(Ordering::Acquire));
    }
}
