mod metadata_filter_support;
use druid::core::{FilterChain, RdbcError};
use druid::pool::DruidPoolBuilder;
use druid_rdbc::connection::Connection;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::statement::StatementOptions;
use druid_wrapper::toasty::ToastyConnectionFactory;
use metadata_filter_support::ExternalMetadataFilter;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn external_metadata_filters_preserve_platform_handle_short_circuit_errors_and_lease_guards()
{
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mode = Arc::new(AtomicU8::new(0));
    let metadata: Arc<dyn ResultSetMetaData> = Arc::new(ResultSetMetaDataSnapshot::new(Vec::new()));
    let mut filters = FilterChain::new();
    for name in ["A", "B", "C"] {
        filters.add_result_set(Arc::new(ExternalMetadataFilter {
            name,
            calls: Arc::clone(&calls),
            metadata: Arc::clone(&metadata),
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
    let mut rows = statement.execute_query("SELECT 1, 'two'").await.unwrap();
    assert_eq!(rows.meta_data().unwrap().column_count(), 2);
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "C:before", "C:ok", "B:ok", "A:ok"]
    );
    calls.lock().unwrap().clear();
    mode.store(1, Ordering::Release);
    assert_eq!(rows.meta_data().unwrap().column_count(), 0);
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:ok", "A:ok"]
    );
    calls.lock().unwrap().clear();
    mode.store(2, Ordering::Release);
    assert_eq!(
        rows.meta_data().map(|_| ()),
        Err(RdbcError::DriverError(
            "external metadata failure".to_owned()
        ))
    );
    assert_eq!(
        *calls.lock().unwrap(),
        ["A:before", "B:before", "B:error", "A:error"]
    );
    rows.close().await.unwrap();
    calls.lock().unwrap().clear();
    assert_eq!(
        rows.meta_data().map(|_| ()),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert!(calls.lock().unwrap().is_empty());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(
        rows.meta_data().map(|_| ()),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert!(calls.lock().unwrap().is_empty());
    pool.close().await;
}
