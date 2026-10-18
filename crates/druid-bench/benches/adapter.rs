//! Adapter borrow and forward-only cursor throughput on real SQLite.

use criterion::{criterion_group, criterion_main, Criterion};
use druid_bench::build_sqlite_pool;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::StatementOptions;
use druid_wrapper::sqlx::SqlxConnectionAdapter;
use std::hint::black_box;

const CURSOR_QUERY: &str = "WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 1000) SELECT n FROM seq";

fn bench_adapter(c: &mut Criterion) {
    let pool = std::sync::Arc::new(build_sqlite_pool("adapter-bench"));

    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("adapter/single_borrow", |b| {
        b.iter(|| {
            let pool = black_box(&pool);
            rt.block_on(async {
                let _ = pool.get_connection().await.unwrap();
            });
        });
    });

    c.bench_function("adapter/parallel_8_borrow", |b| {
        b.iter(|| {
            let pool = pool.clone();
            rt.block_on(async {
                let futs = (0..8).map(|_| {
                    let p = pool.clone();
                    async move {
                        let _ = p.get_connection().await.unwrap();
                    }
                });
                futures::future::join_all(futs).await;
            });
        });
    });

    c.bench_function("adapter/parallel_64_borrow", |b| {
        b.iter(|| {
            let pool = pool.clone();
            rt.block_on(async {
                let futs = (0..64).map(|_| {
                    let p = pool.clone();
                    async move {
                        let _ = p.get_connection().await.unwrap();
                    }
                });
                futures::future::join_all(futs).await;
            });
        });
    });

    // 使用真实 SQLx SQLite 前向游标；连接在两组测量之间复用，语句和游标逐次关闭。
    let mut connection = rt
        .block_on(SqlxConnectionAdapter::connect("sqlite::memory:"))
        .expect("connect SQLite cursor benchmark");
    c.bench_function("adapter/sqlite_cursor_first_row_1000", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut statement = connection
                    .create_statement(StatementOptions::default())
                    .await
                    .expect("create cursor statement");
                statement.set_fetch_size(64).expect("set fetch size");
                let mut rows = statement
                    .execute_query(black_box(CURSOR_QUERY))
                    .await
                    .expect("execute cursor query");
                assert!(rows.next().await.expect("fetch first row"));
                rows.close().await.expect("close cursor");
                statement.close().await.expect("close statement");
            });
        });
    });
    c.bench_function("adapter/sqlite_cursor_full_consume_1000", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut statement = connection
                    .create_statement(StatementOptions::default())
                    .await
                    .expect("create cursor statement");
                statement.set_fetch_size(64).expect("set fetch size");
                let mut rows = statement
                    .execute_query(black_box(CURSOR_QUERY))
                    .await
                    .expect("execute cursor query");
                let mut count = 0;
                while rows.next().await.expect("fetch next row") {
                    count += 1;
                }
                assert_eq!(count, 1000);
                rows.close().await.expect("close cursor");
                statement.close().await.expect("close statement");
            });
        });
    });
    rt.block_on(connection.close())
        .expect("close cursor benchmark connection");
}

criterion_group!(adapter_group, bench_adapter);
criterion_main!(adapter_group);
