//! Shared helpers for druid-rust benchmarks.

#![allow(clippy::all)]

use std::time::Duration;

use druid::pool::{DruidDataSource, DruidPool};

/// Build a minimal SQLite-backed Druid pool for benchmarking.
pub fn build_sqlite_pool(name: &'static str) -> DruidDataSource {
    let mut props = std::collections::HashMap::new();
    props.insert("name".to_owned(), name.to_owned());
    let url = format!("sqlite::file:memdb1_{name}?mode=memory&cache=shared");
    props.insert("url".to_owned(), url);
    props.insert("initialSize".to_owned(), "8".to_owned());
    props.insert("maxActive".to_owned(), "8".to_owned());
    props.insert("minIdle".to_owned(), "8".to_owned());
    props.insert(
        "maxWait".to_owned(),
        Duration::from_secs(5).as_millis().to_string(),
    );
    props.insert("driverClassName".to_owned(), "sqlite".to_owned());
    let factory = std::sync::Arc::new(
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(druid_wrapper::toasty::ToastyConnectionFactory::new(
                "sqlite::file:memdb2_toasty?mode=memory&cache=shared",
            ))
            .expect("ToastyConnectionFactory::new must succeed"),
    );
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(
            druid::pool::DruidDataSourceFactory::create_data_source_with_factory(
                &props, factory, "sqlite",
            ),
        )
        .expect("bench pool must build")
}

/// Borrow a connection (Drop returns it to pool).
pub async fn bench_borrow_return(pool: &DruidPool) {
    let _conn = pool.get_connection().await.expect("borrow");
}
