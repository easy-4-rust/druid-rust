//! Shared helpers for druid-rust benchmarks.

#![allow(clippy::all)]

#[cfg(feature = "wrapper")]
use std::time::Duration;

#[cfg(feature = "wrapper")]
use druid::pool::DruidDataSource;
use druid::pool::DruidPool;

/// 按 `name` 构建 SQLite 基准连接池，返回可用于借还连接测试的数据源。
#[cfg(feature = "wrapper")]
pub fn build_sqlite_pool(name: &'static str) -> DruidDataSource {
    let mut props = std::collections::HashMap::new();
    props.insert("name".to_owned(), name.to_owned());
    let url = "sqlite::memory:".to_owned();
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
                "sqlite::memory:",
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

/// 从 `pool` 借出连接；作用域结束时归还连接。
pub async fn bench_borrow_return(pool: &DruidPool) {
    let _conn = pool.get_connection().await.expect("borrow");
}
