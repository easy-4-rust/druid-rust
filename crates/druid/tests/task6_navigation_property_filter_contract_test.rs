mod navigation_property_filter_support;
use druid::core::{FilterChain, RdbcError};
use druid::pool::DruidPoolBuilder;
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt, ResultSetNavigation};
use druid_rdbc::statement::{FetchDirection, StatementOptions};
use druid_wrapper::toasty::ToastyConnectionFactory;
use navigation_property_filter_support::ExternalNavigationPropertyFilter;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// 外部实现只使用公开标准hook，实际池验证默认、短路、精确错误及租约保护。
#[tokio::test]
async fn external_navigation_property_filters_preserve_real_cursor_and_guards() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mode = Arc::new(AtomicU8::new(0));
    let mut filters = FilterChain::new();
    for name in ["A", "B", "C"] {
        filters.add_result_set(Arc::new(ExternalNavigationPropertyFilter {
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
    let mut rows = statement
        .execute_query("SELECT 10 AS value UNION ALL SELECT 20")
        .await
        .unwrap();
    assert_eq!(rows.navigate(ResultSetNavigation::Last).await, Ok(true));
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "A:nav:Last:before",
            "B:nav:Last:before",
            "C:nav:Last:before",
            "C:nav:ok",
            "B:nav:ok",
            "A:nav:ok"
        ]
    );
    assert_eq!(rows.row().unwrap(), 2);
    rows.set_fetch_size(128).unwrap();
    assert_eq!(rows.fetch_size().unwrap(), 128);
    assert_eq!(
        rows.set_fetch_direction(FetchDirection::Reverse),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_set_fetch_direction"
        })
    );
    assert_eq!(
        rows.fetch_direction(),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_fetch_direction"
        })
    );
    assert!(calls
        .lock()
        .unwrap()
        .iter()
        .any(|event| event == "C:property:SetFetchDirection(Reverse):before"));
    assert_eq!(rows.find_column("value").unwrap().get(), 1);
    calls.lock().unwrap().clear();
    mode.store(1, Ordering::Release);
    assert_eq!(rows.navigate(ResultSetNavigation::Previous).await, Ok(true));
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "A:nav:Previous:before",
            "B:nav:Previous:before",
            "B:nav:ok",
            "A:nav:ok"
        ]
    );
    assert_eq!(rows.row().unwrap(), 77);
    assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 20);
    mode.store(2, Ordering::Release);
    assert_eq!(
        rows.navigate(ResultSetNavigation::Absolute(-1)).await,
        Err(RdbcError::DriverError(
            "ordered navigation failure".to_owned()
        ))
    );
    assert_eq!(
        rows.row(),
        Err(RdbcError::DriverError(
            "ordered property failure".to_owned()
        ))
    );
    mode.store(3, Ordering::Release);
    assert_eq!(
        rows.row(),
        Err(RdbcError::InvalidArgument(
            "incompatible result set property filter value".to_owned()
        ))
    );
    mode.store(0, Ordering::Release);
    assert_eq!(rows.row().unwrap(), 2);
    rows.close().await.unwrap();
    calls.lock().unwrap().clear();
    assert_eq!(rows.row(), Err(RdbcError::ConnectionDiscarded));
    assert_eq!(
        rows.navigate(ResultSetNavigation::Last).await,
        Err(RdbcError::ConnectionDiscarded)
    );
    assert!(rows.is_closed().unwrap());
    assert!(calls.lock().unwrap().is_empty());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(rows.row(), Err(RdbcError::ConnectionDiscarded));
    assert!(calls.lock().unwrap().is_empty());
    pool.close().await;
}
