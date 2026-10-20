//! 对应 Java WallFilterTest：Prepared/Callable 原生游标带有执行 SQL，关闭只汇总一次。
mod standard_test_support;
use druid::core::FilterChain;
use druid::pool::DruidPool;
use druid::sql::{WallConfig, WallFilter, WallProvider};
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{ExecutionResult, PreparedStatement, StatementOptions};
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::Arc;

#[tokio::test]
async fn prepared_and_callable_query_and_generic_results_keep_their_sql_context() {
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    let mut filters = FilterChain::new();
    filters.add_filter(Arc::new(WallFilter::new(Arc::clone(&provider))));
    let pool = DruidPool::builder()
        .factory(Arc::new(TrackingFactory::new(TrackingState::configured())))
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(filters))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    for (callable, generic, table) in [
        (false, false, "prepared_query"),
        (false, true, "prepared_generic"),
        (true, false, "callable_query"),
        (true, true, "callable_generic"),
    ] {
        let sql = format!("SELECT * FROM {table}");
        let mut statement: Box<dyn PreparedStatement> = if callable {
            connection
                .prepare_call(&sql, StatementOptions::default())
                .await
                .unwrap()
        } else {
            connection
                .prepare_statement(&sql, StatementOptions::default())
                .await
                .unwrap()
        };
        let mut rows = if generic {
            let mut results = statement.execute_prepared().await.unwrap();
            assert_eq!(results.len(), 1);
            match results.remove(0) {
                ExecutionResult::ResultSet(rows) => rows,
                _ => panic!("query result expected"),
            }
        } else {
            statement.execute_prepared_query().await.unwrap()
        };
        assert!(rows.next().await.unwrap());
        assert!(!rows.next().await.unwrap());
        assert_eq!(
            provider
                .table_stat_values(false)
                .into_iter()
                .find(|stat| stat.name == table)
                .unwrap()
                .fetch_row_count,
            0
        );
        rows.close().await.unwrap();
        rows.close().await.unwrap();
        statement.close().await.unwrap();
        let table_stat = provider
            .table_stat_values(false)
            .into_iter()
            .find(|stat| stat.name == table)
            .unwrap();
        assert_eq!(table_stat.fetch_row_count, 1, "{table}");
        assert_eq!(
            provider
                .sql_stat(&sql)
                .unwrap()
                .stat_value(false)
                .fetch_row_count,
            1,
            "{sql}"
        );
    }
    connection.close().await.unwrap();
    assert!(provider
        .table_stat_values(false)
        .iter()
        .all(|stat| stat.fetch_row_count == 1));
    pool.close().await;
}
