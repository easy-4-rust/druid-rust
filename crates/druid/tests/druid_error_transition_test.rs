//! P04 `DruidError` 产品所有权与 P03 独立 golden 行为合同。

use std::any::TypeId;
use std::error::Error;
use std::time::Duration;

use druid::core::error::DruidError as ProductDruidError;
use druid_rdbc::error::{RdbcError, SqlException, SqlExceptionCause};
use druid_rdbc::types::RdbcString;

#[derive(Debug, PartialEq, Eq)]
struct SqlObservable {
    error_code: i32,
    sql_state: Option<String>,
    message: Option<String>,
    class_name: String,
    assignable_types: Vec<String>,
    recoverable: bool,
    causes: Vec<SqlExceptionCause>,
    next_exceptions: Vec<SqlObservable>,
}

#[derive(Debug, PartialEq, Eq)]
struct SourceObservable {
    display: String,
    class_name: String,
}

#[derive(Debug, PartialEq, Eq)]
struct ObservableError {
    display: String,
    class_name: String,
    sql_exception: Option<SqlObservable>,
    batch_update_counts: Option<Vec<i32>>,
    source_chain: Vec<SourceObservable>,
}

struct VariantCase {
    name: &'static str,
    value: ProductDruidError,
    expected: ObservableError,
}

fn observe_sql(exception: &SqlException) -> SqlObservable {
    SqlObservable {
        error_code: exception.error_code(),
        sql_state: exception.sql_state().map(str::to_owned),
        message: exception.message().map(str::to_owned),
        class_name: exception.class_name().to_owned(),
        assignable_types: exception.assignable_types().to_vec(),
        recoverable: exception.is_recoverable(),
        causes: exception.causes().to_vec(),
        next_exceptions: exception
            .next_exceptions()
            .iter()
            .map(observe_sql)
            .collect(),
    }
}

fn observe_source_chain(error: &(dyn Error + 'static)) -> Vec<SourceObservable> {
    let mut chain = Vec::new();
    let mut source = error.source();
    while let Some(current) = source {
        let druid_error = current
            .downcast_ref::<ProductDruidError>()
            .expect("DruidError source chain must retain typed DruidError causes");
        chain.push(SourceObservable {
            display: current.to_string(),
            class_name: druid_error.class_name().to_owned(),
        });
        source = current.source();
    }
    chain
}

fn observe(error: &ProductDruidError) -> ObservableError {
    ObservableError {
        display: error.to_string(),
        class_name: error.class_name().to_owned(),
        sql_exception: error.sql_exception().map(|exception| {
            let cloned = exception.clone();
            assert_eq!(
                &cloned, exception,
                "SqlException clone must preserve all fields"
            );
            observe_sql(&cloned)
        }),
        batch_update_counts: error
            .batch_update_counts()
            .map(druid_rdbc::error::BatchUpdateCounts::jdbc_i32),
        source_chain: observe_source_chain(error),
    }
}

fn golden(
    display: &str,
    class_name: &str,
    sql_exception: Option<SqlObservable>,
    batch_update_counts: Option<Vec<i32>>,
    source_chain: Vec<SourceObservable>,
) -> ObservableError {
    ObservableError {
        display: display.to_owned(),
        class_name: class_name.to_owned(),
        sql_exception,
        batch_update_counts,
        source_chain,
    }
}

fn source(display: &str, class_name: &str) -> SourceObservable {
    SourceObservable {
        display: display.to_owned(),
        class_name: class_name.to_owned(),
    }
}

fn assert_exhaustive_variant(error: &ProductDruidError, expected_name: &str) {
    let actual_name = match error {
        ProductDruidError::PoolClosed => "PoolClosed",
        ProductDruidError::DataSourceClosed { .. } => "DataSourceClosed",
        ProductDruidError::AcquireTimeout => "AcquireTimeout",
        ProductDruidError::GetConnectionTimeout { .. } => "GetConnectionTimeout",
        ProductDruidError::LoginTimeout => "LoginTimeout",
        ProductDruidError::PoolExhausted => "PoolExhausted",
        ProductDruidError::MaxWaitThreadCountExceeded { .. } => "MaxWaitThreadCountExceeded",
        ProductDruidError::ValidationFailed(_) => "ValidationFailed",
        ProductDruidError::ConnectionLeaked { .. } => "ConnectionLeaked",
        ProductDruidError::ConnectionDiscarded => "ConnectionDiscarded",
        ProductDruidError::DriverError(_) => "DriverError",
        ProductDruidError::SqlException(_) => "SqlException",
        ProductDruidError::BatchUpdateException { .. } => "BatchUpdateException",
        ProductDruidError::SqlParseError(_) => "SqlParseError",
        ProductDruidError::WallViolation(_) => "WallViolation",
        ProductDruidError::DataSourceNotFound(_) => "DataSourceNotFound",
        ProductDruidError::DataSourceDisabled => "DataSourceDisabled",
        ProductDruidError::ActiveConnectionsPreventRestart { .. } => {
            "ActiveConnectionsPreventRestart"
        }
        ProductDruidError::OnFatalError {
            active_count,
            max_active,
            last_error_time_millis,
            last_sql,
            cause,
        } => {
            assert_eq!(*active_count, 13);
            assert_eq!(*max_active, 12);
            assert_eq!(*last_error_time_millis, 0);
            assert_eq!(
                last_sql.as_ref().map(RdbcString::as_utf16),
                Some(&[0x0041, 0xD800, 0x0042][..])
            );
            assert!(cause.is_some());
            "OnFatalError"
        }
        ProductDruidError::DataSourceNotAvailable { .. } => "DataSourceNotAvailable",
        ProductDruidError::InvalidArgument(_) => "InvalidArgument",
        ProductDruidError::UnsupportedOperation { .. } => "UnsupportedOperation",
        ProductDruidError::NoDriverExtension { .. } => "NoDriverExtension",
        ProductDruidError::Other(_) => "Other",
        ProductDruidError::Rdbc(_) => "Rdbc",
    };
    assert_eq!(actual_name, expected_name);
}

#[test]
fn product_error_is_distinct_and_wraps_rdbc_once() {
    assert_ne!(TypeId::of::<ProductDruidError>(), TypeId::of::<RdbcError>());

    let product =
        ProductDruidError::from(RdbcError::DriverError("driver boundary failed".to_owned()));
    assert!(matches!(
        product,
        ProductDruidError::Rdbc(RdbcError::DriverError(ref message))
            if message == "driver boundary failed"
    ));
}

#[test]
fn wrapped_rdbc_batch_counts_delegate_all_three_views() {
    let large = i64::from(i32::MAX) + 7;
    let product = ProductDruidError::Rdbc(RdbcError::BatchUpdateException {
        update_counts: vec![1_i64, large, -2, -3].into(),
        cause: Box::new(RdbcError::DriverError("batch failed".to_owned())),
    });

    let counts = product
        .batch_update_counts()
        .expect("DruidError::Rdbc must expose wrapped batch counts");
    assert_eq!(counts.large(), &[1, large, -2, -3]);
    assert_eq!(counts.jdbc_i32(), vec![1, large as i32, -2, -3]);
    assert!(counts.try_i32().is_err());
}

#[test]
fn all_24_variants_match_independently_derived_golden_observables() {
    let mut sql_exception = SqlException::new(
        1205,
        Some("40001".to_owned()),
        Some("deadlock victim".to_owned()),
    )
    .with_class_name("vendor.DeadlockException")
    .with_assignable_type("vendor.DatabaseException")
    .recoverable()
    .with_cause(SqlExceptionCause::SocketTimeout)
    .with_cause(SqlExceptionCause::ClassName("vendor.RootCause".to_owned()));
    sql_exception.set_next_exception(SqlException::driver(1213, "retry transaction"));
    sql_exception.set_next_exception(
        SqlException::new(
            1214,
            Some("40002".to_owned()),
            Some("retry exhausted".to_owned()),
        )
        .with_class_name("vendor.RetryExhaustedException")
        .with_assignable_type("vendor.DatabaseException")
        .with_cause(SqlExceptionCause::ClassName(
            "vendor.RetryRootCause".to_owned(),
        )),
    );

    let batch_sql_exception = SqlException::new(
        1213,
        Some("40001".to_owned()),
        Some("batch retry".to_owned()),
    )
    .with_class_name("vendor.BatchException")
    .with_assignable_type("vendor.DatabaseException")
    .with_cause(SqlExceptionCause::SocketTimeout);

    let cases = vec![
        VariantCase {
            name: "PoolClosed",
            value: ProductDruidError::PoolClosed,
            expected: golden(
                "connection pool is closed",
                "druid::PoolClosed",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "DataSourceClosed",
            value: ProductDruidError::DataSourceClosed {
                close_time_millis: u64::MAX,
            },
            expected: golden(
                "dataSource already closed at 18446744073709551615",
                "com.alibaba.druid.pool.DataSourceClosedException",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "AcquireTimeout",
            value: ProductDruidError::AcquireTimeout,
            expected: golden(
                "acquire connection timed out",
                "druid::AcquireTimeout",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "GetConnectionTimeout",
            value: ProductDruidError::GetConnectionTimeout {
                wait_millis: 5_000,
                active_count: 8,
                max_active: 8,
                creating_count: 1,
                create_elapsed_millis: Some(750),
                create_error_count: 2,
                running_sql: vec![(3, "select 1".to_owned()), (2, "select 2".to_owned())],
                cause: Some(Box::new(ProductDruidError::DriverError(
                    "connect refused".to_owned(),
                ))),
            },
            expected: golden(
                "wait millis 5000, active 8, maxActive 8, creating 1, createElapseMillis 750, createErrorCount 2, runningSqlCount 3 : select 1\nrunningSqlCount 2 : select 2",
                "com.alibaba.druid.pool.GetConnectionTimeoutException",
                None,
                None,
                vec![source("driver error: connect refused", "druid::DriverError")],
            ),
        },
        VariantCase {
            name: "LoginTimeout",
            value: ProductDruidError::LoginTimeout,
            expected: golden(
                "physical connection login timed out",
                "druid::LoginTimeout",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "PoolExhausted",
            value: ProductDruidError::PoolExhausted,
            expected: golden(
                "connection pool exhausted",
                "druid::PoolExhausted",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "MaxWaitThreadCountExceeded",
            value: ProductDruidError::MaxWaitThreadCountExceeded {
                max: 16,
                current: 17,
            },
            expected: golden(
                "maxWaitThreadCount 16, current wait task count 17",
                "druid::MaxWaitThreadCountExceeded",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "ValidationFailed",
            value: ProductDruidError::ValidationFailed("validation query failed".to_owned()),
            expected: golden(
                "connection validation failed: validation query failed",
                "druid::ValidationFailed",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "ConnectionLeaked",
            value: ProductDruidError::ConnectionLeaked {
                id: 42,
                held_for: Duration::from_secs(30),
            },
            expected: golden(
                "connection 42 leaked, held for 30s",
                "druid::ConnectionLeaked",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "ConnectionDiscarded",
            value: ProductDruidError::ConnectionDiscarded,
            expected: golden(
                "connection has been discarded",
                "druid::ConnectionDiscarded",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "DriverError",
            value: ProductDruidError::DriverError("driver failed".to_owned()),
            expected: golden(
                "driver error: driver failed",
                "druid::DriverError",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "SqlException",
            value: ProductDruidError::SqlException(Box::new(sql_exception)),
            expected: golden(
                "SQL exception (code=1205, state=40001): deadlock victim",
                "vendor.DeadlockException",
                Some(SqlObservable {
                    error_code: 1205,
                    sql_state: Some("40001".to_owned()),
                    message: Some("deadlock victim".to_owned()),
                    class_name: "vendor.DeadlockException".to_owned(),
                    assignable_types: vec![
                        "vendor.DeadlockException".to_owned(),
                        "java.sql.SQLException".to_owned(),
                        "vendor.DatabaseException".to_owned(),
                    ],
                    recoverable: true,
                    causes: vec![
                        SqlExceptionCause::SocketTimeout,
                        SqlExceptionCause::ClassName("vendor.RootCause".to_owned()),
                    ],
                    next_exceptions: vec![
                        SqlObservable {
                            error_code: 1213,
                            sql_state: None,
                            message: Some("retry transaction".to_owned()),
                            class_name: "java.sql.SQLException".to_owned(),
                            assignable_types: vec!["java.sql.SQLException".to_owned()],
                            recoverable: false,
                            causes: vec![],
                            next_exceptions: vec![],
                        },
                        SqlObservable {
                            error_code: 1214,
                            sql_state: Some("40002".to_owned()),
                            message: Some("retry exhausted".to_owned()),
                            class_name: "vendor.RetryExhaustedException".to_owned(),
                            assignable_types: vec![
                                "vendor.RetryExhaustedException".to_owned(),
                                "java.sql.SQLException".to_owned(),
                                "vendor.DatabaseException".to_owned(),
                            ],
                            recoverable: false,
                            causes: vec![SqlExceptionCause::ClassName(
                                "vendor.RetryRootCause".to_owned(),
                            )],
                            next_exceptions: vec![],
                        },
                    ],
                }),
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "BatchUpdateException",
            value: ProductDruidError::BatchUpdateException {
                update_counts: vec![1_i32, -2, -3].into(),
                cause: Box::new(ProductDruidError::SqlException(Box::new(
                    batch_sql_exception,
                ))),
            },
            expected: golden(
                "batch update failed after 3 result(s): SQL exception (code=1213, state=40001): batch retry",
                "java.sql.BatchUpdateException",
                Some(SqlObservable {
                    error_code: 1213,
                    sql_state: Some("40001".to_owned()),
                    message: Some("batch retry".to_owned()),
                    class_name: "vendor.BatchException".to_owned(),
                    assignable_types: vec![
                        "vendor.BatchException".to_owned(),
                        "java.sql.SQLException".to_owned(),
                        "vendor.DatabaseException".to_owned(),
                    ],
                    recoverable: false,
                    causes: vec![SqlExceptionCause::SocketTimeout],
                    next_exceptions: vec![],
                }),
                Some(vec![1, -2, -3]),
                vec![source(
                    "SQL exception (code=1213, state=40001): batch retry",
                    "vendor.BatchException",
                )],
            ),
        },
        VariantCase {
            name: "SqlParseError",
            value: ProductDruidError::SqlParseError("unexpected token".to_owned()),
            expected: golden(
                "SQL parse error: unexpected token",
                "druid::SqlParseError",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "WallViolation",
            value: ProductDruidError::WallViolation("blocked statement".to_owned()),
            expected: golden(
                "wall violation: blocked statement",
                "druid::WallViolation",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "DataSourceNotFound",
            value: ProductDruidError::DataSourceNotFound("orders".to_owned()),
            expected: golden(
                "datasource not found: orders",
                "druid::DataSourceNotFound",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "DataSourceDisabled",
            value: ProductDruidError::DataSourceDisabled,
            expected: golden(
                "datasource is disabled",
                "com.alibaba.druid.pool.DataSourceDisableException",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "ActiveConnectionsPreventRestart",
            value: ProductDruidError::ActiveConnectionsPreventRestart { active_count: 5 },
            expected: golden(
                "can not restart, active connection count not zero: 5",
                "druid::ActiveConnectionsPreventRestart",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "OnFatalError",
            value: ProductDruidError::OnFatalError {
                active_count: 13,
                max_active: 12,
                last_error_time_millis: 0,
                last_sql: Some(RdbcString::from_utf16([0x0041, 0xD800, 0x0042])),
                cause: Some(Box::new(ProductDruidError::DataSourceNotAvailable {
                    cause: Some(Box::new(ProductDruidError::Other(
                        "nested fatal cause".to_owned(),
                    ))),
                })),
            },
            expected: golden(
                "onFatalError, activeCount 13, onFatalErrorMaxActive 12, sql \nA�B",
                "java.sql.SQLException",
                None,
                None,
                vec![
                    source(
                        "nested fatal cause",
                        "druid::DataSourceNotAvailable",
                    ),
                    source("nested fatal cause", "druid::Other"),
                ],
            ),
        },
        VariantCase {
            name: "DataSourceNotAvailable",
            value: ProductDruidError::DataSourceNotAvailable {
                cause: Some(Box::new(ProductDruidError::Other(
                    "availability cause".to_owned(),
                ))),
            },
            expected: golden(
                "availability cause",
                "druid::DataSourceNotAvailable",
                None,
                None,
                vec![source("availability cause", "druid::Other")],
            ),
        },
        VariantCase {
            name: "InvalidArgument",
            value: ProductDruidError::InvalidArgument("bad argument".to_owned()),
            expected: golden(
                "invalid argument: bad argument",
                "druid::InvalidArgument",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "UnsupportedOperation",
            value: ProductDruidError::UnsupportedOperation { operation: "lob" },
            expected: golden(
                "operation is not supported by the physical connection: lob",
                "druid::UnsupportedOperation",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "NoDriverExtension",
            value: ProductDruidError::NoDriverExtension {
                db_type: "postgresql".to_owned(),
            },
            expected: golden(
                "no driver extension registered for database type: postgresql",
                "druid::NoDriverExtension",
                None,
                None,
                vec![],
            ),
        },
        VariantCase {
            name: "Other",
            value: ProductDruidError::Other("other failure".to_owned()),
            expected: golden("other failure", "druid::Other", None, None, vec![]),
        },
    ];

    assert_eq!(cases.len(), 24);
    for case in cases {
        assert_exhaustive_variant(&case.value, case.name);
        assert_eq!(observe(&case.value), case.expected, "{}", case.name);
    }
}

#[test]
fn string_conversions_match_independent_other_variant_goldens() {
    let from_owned: ProductDruidError = String::from("owned failure").into();
    assert_eq!(
        observe(&from_owned),
        golden("owned failure", "druid::Other", None, None, vec![],)
    );
    assert!(matches!(
        from_owned,
        ProductDruidError::Other(ref message) if message == "owned failure"
    ));

    let from_borrowed: ProductDruidError = "borrowed failure".into();
    assert_eq!(
        observe(&from_borrowed),
        golden("borrowed failure", "druid::Other", None, None, vec![],)
    );
    assert!(matches!(
        from_borrowed,
        ProductDruidError::Other(ref message) if message == "borrowed failure"
    ));
}
