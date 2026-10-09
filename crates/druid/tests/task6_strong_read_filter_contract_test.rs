mod strong_read_filter_support;
use druid::core::{FilterChain, RdbcError};
use druid::pool::DruidPoolBuilder;
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::RdbcCalendarArgument;
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::{Arc, Mutex};
use strong_read_filter_support::OrderedReadFilter;

#[tokio::test]
async fn external_n_string_filters_keep_descriptor_short_circuit_null_and_default_order() {
    use strong_read_filter_support::NStringReadFilter;
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    for (name, short_circuit) in [("a", false), ("b", true)] {
        filters.add_result_set(Arc::new(NStringReadFilter {
            name,
            short_circuit,
            log: Arc::clone(&log),
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
        .execute_query("SELECT '你好，Druid😀' AS value UNION ALL SELECT NULL")
        .await
        .unwrap();
    assert_eq!(
        rows.n_string_by_label("virtual").unwrap(),
        Some("外部短路😀".to_owned())
    );
    let short_order = vec![
        "a:read:NString:before",
        "b:read:NString:before",
        "b:read:NString:after",
        "a:read:NString:after",
    ];
    assert_eq!(*log.lock().unwrap(), short_order);
    log.lock().unwrap().clear();
    assert_eq!(
        rows.n_string_by_label("fail"),
        Err(RdbcError::DriverError(
            "external NString failure".to_owned()
        ))
    );
    assert_eq!(*log.lock().unwrap(), short_order);
    log.lock().unwrap().clear();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.n_string_by_label("value").unwrap(),
        Some("你好，Druid😀".to_owned())
    );
    let default_order = vec![
        "a:read:NString:before",
        "b:read:NString:before",
        "b:read:NString:after",
        "a:read:NString:after",
    ];
    assert_eq!(*log.lock().unwrap(), default_order);
    log.lock().unwrap().clear();
    assert_eq!(
        rows.string(ColumnIndex::new(1).unwrap()).unwrap(),
        Some("你好，Druid😀".to_owned())
    );
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "a:read:String:before",
            "b:read:String:before",
            "a:value:before",
            "b:value:before",
            "b:value:after",
            "a:value:after",
            "b:read:String:after",
            "a:read:String:after"
        ]
    );
    log.lock().unwrap().clear();
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.n_string(ColumnIndex::new(1).unwrap()).unwrap(), None);
    assert!(rows.was_null().unwrap());
    assert_eq!(*log.lock().unwrap(), default_order);
    rows.close().await.unwrap();
    log.lock().unwrap().clear();
    assert_eq!(
        rows.n_string_by_label("virtual"),
        Err(RdbcError::ConnectionDiscarded)
    );
    assert!(log.lock().unwrap().is_empty());
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn external_filters_preserve_short_circuit_default_delegation_and_error_unwind() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut filters = FilterChain::new();
    filters.add_result_set(Arc::new(OrderedReadFilter {
        name: "a",
        short_circuit: false,
        log: Arc::clone(&log),
    }));
    filters.add_result_set(Arc::new(OrderedReadFilter {
        name: "b",
        short_circuit: true,
        log: Arc::clone(&log),
    }));
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
    let mut rows = statement.execute_query("SELECT 1 AS value").await.unwrap();
    let expected = chrono::NaiveDate::from_ymd_opt(2031, 1, 3)
        .unwrap()
        .and_hms_opt(3, 4, 5)
        .unwrap();
    assert_eq!(
        rows.timestamp_by_label("virtual", &RdbcCalendarArgument::Specified(None))
            .unwrap(),
        Some(expected)
    );
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "a:read:before",
            "b:read:before",
            "b:read:after",
            "a:read:after"
        ]
    );
    log.lock().unwrap().clear();
    assert_eq!(
        rows.timestamp_by_label("fail", &RdbcCalendarArgument::Specified(None)),
        Err(RdbcError::DriverError(
            "external filtered failure".to_owned()
        ))
    );
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "a:read:before",
            "b:read:before",
            "b:read:after",
            "a:read:after"
        ]
    );
    log.lock().unwrap().clear();
    assert!(rows.next().await.unwrap());
    assert_eq!(rows.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    assert_eq!(
        *log.lock().unwrap(),
        vec![
            "a:read:before",
            "b:read:before",
            "a:value:before",
            "b:value:before",
            "b:value:after",
            "a:value:after",
            "b:read:after",
            "a:read:after"
        ]
    );
    rows.close().await.unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}
