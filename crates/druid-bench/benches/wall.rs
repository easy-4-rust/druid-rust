//! Wall visitor throughput.

use criterion::{criterion_group, criterion_main, Criterion};
use druid::sql::WallConfig;
use std::sync::Arc;

fn bench_wall(c: &mut Criterion) {
    let provider = Arc::new(druid::sql::WallProvider::new(WallConfig::default()));

    let cases: Vec<&str> = vec![
        "SELECT id, name FROM users WHERE id = ?",
        "INSERT INTO logs (msg, ts) VALUES (?, ?)",
        "UPDATE users SET name = ? WHERE id = ?",
        "DELETE FROM sessions WHERE user_id = ?",
        "DROP TABLE temp_data",
        "TRUNCATE TABLE logs",
    ];

    c.bench_function("wall/check_2_safe", |b| {
        b.iter(|| {
            let provider = provider.clone();
            for sql in &cases[..2] {
                let _ = provider.check(sql);
            }
        });
    });

    c.bench_function("wall/check_all_categories", |b| {
        b.iter(|| {
            let provider = provider.clone();
            for sql in &cases {
                let _ = provider.check(sql);
            }
        });
    });
}

criterion_group!(wall_group, bench_wall);
criterion_main!(wall_group);
