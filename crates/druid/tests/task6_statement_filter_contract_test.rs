mod statement_filter_support;
use druid::core::{FilterChain, RdbcError};
use druid::pool::DruidPoolBuilder;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::StatementOptions;
use druid_wrapper::toasty::ToastyConnectionFactory;
use statement_filter_support::ExternalStatementFilter;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn external_statement_filters_preserve_standard_identity_short_circuit_and_lease_guards() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mode = Arc::new(AtomicU8::new(0));
    let mut filters = FilterChain::new();
    for name in ["A", "B", "C"] {
        filters.add_result_set(Arc::new(ExternalStatementFilter {
            name,
            calls: Arc::clone(&calls),
            mode: if name == "B" {
                Arc::clone(&mode)
            } else {
                Arc::new(AtomicU8::new(0))
            },
        }));
    }
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPoolBuilder::new()
        .factory(Arc::new(factory))
        .filter_chain(Arc::new(filters))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement.execute_query("SELECT 1").await.unwrap();
    statement.set_fetch_size(13).unwrap();
    let mut owner = rows.get_statement().unwrap().unwrap();
    assert_eq!(owner.fetch_size().unwrap(), 13);
    owner.set_fetch_size(23).unwrap();
    assert_eq!(statement.fetch_size().unwrap(), 23);
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "C:before", "C:ok", "B:ok", "A:ok"]
    );
    drop(owner);
    calls.lock().unwrap().clear();
    mode.store(1, Ordering::Release);
    assert!(rows.get_statement().unwrap().is_none());
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
    );
    calls.lock().unwrap().clear();
    mode.store(2, Ordering::Release);
    assert_eq!(
        rows.get_statement().map(|_| ()),
        Err(RdbcError::DriverError(
            "external statement failure".to_owned()
        ))
    );
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:error", "A:error"]
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    calls.lock().unwrap().clear();
    assert_eq!(
        rows.get_statement().map(|_| ()),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert!(calls.lock().unwrap().is_empty());
    pool.close().await;
}
