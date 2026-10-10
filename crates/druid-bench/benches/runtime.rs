//! Measure pool overload and shutdown behavior under a fixed burst.

use criterion::{criterion_group, criterion_main, Criterion};
use druid_bench::build_sqlite_pool;
use futures::future::join_all;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn runtime_benchmarks(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().expect("Tokio runtime");
    let pool = Arc::new(build_sqlite_pool("runtime-overload"));

    c.bench_function("runtime/pool_burst_64", |bench| {
        bench.iter(|| {
            let pool = Arc::clone(&pool);
            runtime.block_on(async {
                let futures = (0..64).map(|_| {
                    let pool = Arc::clone(&pool);
                    async move {
                        let connection = pool.get_connection().await.expect("borrow");
                        tokio::time::sleep(Duration::from_millis(2)).await;
                        drop(connection);
                    }
                });
                join_all(futures).await;
            });
            std::hint::black_box(pool.state());
        });
    });

    let state = pool.state();
    let shutdown_started = Instant::now();
    runtime.block_on(pool.close());
    eprintln!(
        "runtime_benchmark active_peak={} waits={} shutdown_ms={}",
        state.active_peak,
        state.not_empty_wait_count,
        shutdown_started.elapsed().as_millis(),
    );
}

criterion_group!(runtime_group, runtime_benchmarks);
criterion_main!(runtime_group);
