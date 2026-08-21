//! Toasty adapter throughput (real SQLite).

use criterion::{criterion_group, criterion_main, Criterion};
use druid_bench::build_sqlite_pool;
use std::hint::black_box;

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
}

criterion_group!(adapter_group, bench_adapter);
criterion_main!(adapter_group);
