//! FilterChain throughput.

use criterion::{criterion_group, criterion_main, Criterion};
use druid::core::{FilterAdapter, FilterChain};
use std::sync::Arc;

fn bench_filter_chain(c: &mut Criterion) {
    c.bench_function("filter_chain/empty_build", |b| {
        b.iter(|| {
            let chain = FilterChain::new();
            let _ = chain.filter_class_names();
        });
    });

    c.bench_function("filter_chain/5_adapter_build", |b| {
        b.iter(|| {
            let mut chain = FilterChain::new();
            for _ in 0..5 {
                chain.add_filter(Arc::new(FilterAdapter::new()));
            }
            let _ = chain.filter_class_names();
        });
    });

    c.bench_function("filter_chain/20_adapter_build", |b| {
        b.iter(|| {
            let mut chain = FilterChain::new();
            for _ in 0..20 {
                chain.add_filter(Arc::new(FilterAdapter::new()));
            }
            let _ = chain.filter_class_names();
        });
    });
}

criterion_group!(filter_group, bench_filter_chain);
criterion_main!(filter_group);
