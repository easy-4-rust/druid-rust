//! 使用真实 SQLite/H2 池化执行核对 Java WallFilterTest 的表统计语义。
#![cfg(any(feature = "sqlx", feature = "jdbc-agent"))]

use druid::core::FilterChain;
use druid::pool::{DruidPool, PoolInnerConfig};
use druid::sql::{WallConfig, WallFilter, WallProvider};
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::statement::{GeneratedKeys, ParameterIndex, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
#[cfg(feature = "jdbc-agent")]
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
#[cfg(feature = "sqlx")]
use druid_wrapper::sqlx::SqlxConnectionFactory;
use std::sync::Arc;

/// 对应 Java: com.alibaba.druid.bvt.filter.wall.WallFilterTest#test_wallFilter。
#[cfg(feature = "sqlx")]
#[tokio::test]
async fn pooled_wall_table_counts_follow_real_sqlite_results() {
    verify_wall_table_counts(
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
        false,
    )
    .await;
}

/// H2 路径补齐 Java 原用例的 TRUNCATE 和 prepared batch。
#[cfg(feature = "jdbc-agent")]
#[tokio::test]
async fn pooled_wall_table_counts_follow_real_h2_results() {
    let Some(agent) = std::env::var_os("DRUID_JDBC_AGENT_JAR") else {
        return;
    };
    let Some(driver) = std::env::var_os("DRUID_JDBC_DRIVER_JAR") else {
        return;
    };
    let options = JdbcAgentOptions::java("java", [agent.into(), driver.into()])
        .expect("valid Agent classpath");
    verify_wall_table_counts(
        Arc::new(JdbcAgentConnectionFactory::new(
            "jdbc:h2:mem:wall_table_stats;DB_CLOSE_DELAY=-1",
            Some("SELECT 1".to_owned()),
            options.runtime_identity("wall-table-stats", "local", "isolated"),
        )),
        true,
    )
    .await;
}

async fn verify_wall_table_counts(factory: Arc<dyn ConnectionFactory>, truncate: bool) {
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(WallFilter::new(Arc::clone(&provider))));
    let pool = DruidPool::new_standard(
        "wall-table-stats".to_owned(),
        "sqlite".to_owned(),
        factory,
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let table_stat = || {
        provider
            .table_stat_values(false)
            .into_iter()
            .find(|stat| stat.name.eq_ignore_ascii_case("t"))
            .expect("table stat")
    };
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
    assert_eq!(table_stat().create_count, 1);
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
    assert_eq!(table_stat().fetch_row_count, 10);
    assert_eq!(
        statement
            .execute_update("DELETE FROM t WHERE fid = 0", GeneratedKeys::None)
            .await
            .unwrap(),
        1
    );
    assert_eq!(table_stat().delete_data_count, 1);
    assert_eq!(
        statement
            .execute_update("DELETE FROM t WHERE fid IN (1, 2)", GeneratedKeys::None)
            .await
            .unwrap(),
        2
    );
    assert_eq!(table_stat().delete_data_count, 3);
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
    assert_eq!(table_stat().update_data_count, 2);

    let mut prepared = connection
        .prepare_statement(
            "UPDATE t SET fname = 'yy' WHERE fid = ? OR fid = ?",
            StatementOptions::default(),
        )
        .await
        .expect("prepared update");
    for expected in [4, 6] {
        for (index, id) in [(1, 3), (2, 4)] {
            prepared
                .set_parameter(PreparedParameter {
                    index: ParameterIndex::new(index).unwrap(),
                    value: Value::Int(id),
                })
                .expect("bind id");
        }
        assert_eq!(
            prepared
                .execute_prepared_update()
                .await
                .expect("prepared update"),
            2
        );
        assert_eq!(table_stat().update_data_count, expected);
    }
    prepared.close().await.expect("close prepared");
    let mut batch = connection
        .prepare_statement(
            "UPDATE t SET fname = 'zz' WHERE fid = ?",
            StatementOptions::default(),
        )
        .await
        .expect("batch update");
    for id in [3, 4] {
        batch
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(1).unwrap(),
                value: Value::Int(id),
            })
            .expect("bind batch id");
        batch.add_prepared_batch().expect("add prepared batch");
    }
    assert_eq!(
        batch.execute_prepared_batch().await.expect("execute batch"),
        vec![1, 1]
    );
    assert_eq!(table_stat().update_data_count, 8);
    batch.close().await.expect("close batch");
    if truncate {
        statement
            .execute_update("TRUNCATE TABLE t", GeneratedKeys::None)
            .await
            .expect("truncate table");
        assert_eq!(table_stat().truncate_count, 1);
    }
    statement
        .execute_update("DROP TABLE t", GeneratedKeys::None)
        .await
        .expect("drop table");
    let table = table_stat();
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
    assert_eq!(table.update_data_count, 8);
    assert_eq!(table.truncate_count, u64::from(truncate));
    assert_eq!(table.drop_count, 1);
    statement.close().await.expect("close statement");
    connection.close().await.expect("close connection");
    pool.close().await;
}

/// 两条不同 SQL 的批次返回值必须回写各自的表统计，不能错位。
#[cfg(feature = "sqlx")]
#[tokio::test]
async fn wall_statement_batch_counts_are_associated_with_each_sql() {
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    let mut chain = FilterChain::new();
    chain.add_filter(Arc::new(WallFilter::new(Arc::clone(&provider))));
    let pool = DruidPool::new_standard(
        "wall-statement-batch".to_owned(),
        "sqlite".to_owned(),
        Arc::new(SqlxConnectionFactory::new("sqlite::memory:")),
        PoolInnerConfig::default(),
        Some(Arc::new(chain)),
    );
    pool.init().await.expect("pool init");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("statement");
    for sql in [
        "CREATE TABLE updates (id INTEGER, value TEXT)",
        "CREATE TABLE deletes (id INTEGER)",
        "INSERT INTO updates VALUES (1, 'old')",
        "INSERT INTO deletes VALUES (1)",
    ] {
        statement
            .execute_update(sql, GeneratedKeys::None)
            .await
            .expect("setup SQL");
    }
    statement
        .add_batch("UPDATE updates SET value = 'new' WHERE id = 1".to_owned())
        .expect("add update");
    statement
        .add_batch("DELETE FROM deletes WHERE id = 1".to_owned())
        .expect("add delete");
    assert_eq!(
        statement.execute_batch().await.expect("execute batch"),
        vec![1, 1]
    );
    let stats = provider.table_stat_values(false);
    let updates = stats.iter().find(|stat| stat.name == "updates").unwrap();
    let deletes = stats.iter().find(|stat| stat.name == "deletes").unwrap();
    assert_eq!(updates.update_data_count, 1);
    assert_eq!(updates.delete_data_count, 0);
    assert_eq!(deletes.update_data_count, 0);
    assert_eq!(deletes.delete_data_count, 1);
    statement.close().await.expect("close statement");
    connection.close().await.expect("close connection");
    pool.close().await;
}
