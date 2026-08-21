//! Pool borrow/return throughput.

use criterion::{criterion_group, criterion_main, Criterion};
use druid_bench::build_sqlite_pool;
use futures::future::join_all;
use std::sync::Arc;

fn bench_pool(c: &mut Criterion) {
    let pool = Arc::new(build_sqlite_pool("bench"));
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("pool/single_borrow_return", |b| {
        b.iter(|| {
            let pool = pool.clone();
            rt.block_on(async {
                let _ = pool.get_connection().await.unwrap();
            });
        });
    });

    c.bench_function("pool/parallel_16", |b| {
        b.iter(|| {
            let pool = pool.clone();
            rt.block_on(async {
                let futs = (0..16).map(|_| {
                    let p = pool.clone();
                    async move {
                        let _ = p.get_connection().await.unwrap();
                    }
                });
                join_all(futs).await;
            });
        });
    });

    c.bench_function("pool/parallel_64", |b| {
        b.iter(|| {
            let pool = pool.clone();
            rt.block_on(async {
                let futs = (0..64).map(|_| {
                    let p = pool.clone();
                    async move {
                        let _ = p.get_connection().await.unwrap();
                    }
                });
                join_all(futs).await;
            });
        });
    });
}

criterion_group!(pool_group, bench_pool);
criterion_main!(pool_group);
