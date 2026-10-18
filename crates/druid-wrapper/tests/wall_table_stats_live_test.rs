//! 使用真实 SQLite 池化执行核对 Java WallFilterTest 的表统计语义。
#![cfg(feature = "sqlx")]

use druid::core::FilterChain;
use druid::pool::{DruidPool, PoolInnerConfig};
use druid::sql::{WallConfig, WallFilter, WallProvider};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::statement::{GeneratedKeys, ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionFactory;
use std::sync::Arc;

/// 对应 Java: com.alibaba.druid.bvt.filter.wall.WallFilterTest#test_wallFilter。
#[tokio::test]
async fn pooled_wall_table_counts_follow_real_sqlite_results() {
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(WallFilter::new(Arc::clone(&provider))));
    let pool = DruidPool::new_standard(
        "wall-table-stats".to_owned(),
        "sqlite".to_owned(),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")) as Arc<dyn ConnectionFactory>,
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("statement");
    statement
        .execute_update(
            "CREATE TABLE t (fid INTEGER, fname TEXT)",
            GeneratedKeys::None,
        )
        .await
        .expect("create table");
    for id in 0..10 {
        statement
            .execute_update(
                &format!("INSERT INTO t (fid, fname) VALUES ({id}, 'a{id}')"),
                GeneratedKeys::None,
            )
            .await
            .expect("insert row");
    }
    let mut rows = statement
        .execute_query("SELECT * FROM t")
        .await
        .expect("select rows");
    let mut observed = 0;
    while rows.next().await.expect("next row") {
        observed += 1;
    }
    assert_eq!(observed, 10);
    rows.close().await.expect("close rows");
    assert_eq!(
        statement
            .execute_update("DELETE FROM t WHERE fid = 0", GeneratedKeys::None)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        statement
            .execute_update("DELETE FROM t WHERE fid IN (1, 2)", GeneratedKeys::None)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        statement
            .execute_update(
                "UPDATE t SET fname = 'xx' WHERE fid IN (3, 4)",
                GeneratedKeys::None
            )
            .await
            .unwrap(),
        2
    );

    let mut prepared = connection
        .prepare_statement(
            "UPDATE t SET fname = 'yy' WHERE fid = ?",
            StatementOptions::default(),
        )
        .await
        .expect("prepared update");
    for id in [3, 4] {
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::Int(id),
            })
            .expect("bind id");
        assert_eq!(
            prepared
                .execute_prepared_update()
                .await
                .expect("prepared update"),
            1
        );
    }
    prepared.close().await.expect("close prepared");
    statement
        .execute_update("DROP TABLE t", GeneratedKeys::None)
        .await
        .expect("drop table");
    let stats = provider.table_stat_values(false);
    let table = stats
        .iter()
        .find(|stat| stat.name.eq_ignore_ascii_case("t"))
        .expect("table stat");
    assert_eq!(table.create_count, 1);
    assert_eq!(table.fetch_row_count, 10);
    assert_eq!(
        provider
            .sql_stat("SELECT * FROM t")
            .expect("select SQL stat")
            .stat_value(false)
            .fetch_row_count,
        10
    );
    assert_eq!(table.delete_data_count, 3);
    assert_eq!(table.update_data_count, 4);
    assert_eq!(table.drop_count, 1);
    statement.close().await.expect("close statement");
    connection.close().await.expect("close connection");
    pool.close().await;
}
