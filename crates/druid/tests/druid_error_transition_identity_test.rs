//! P03 transitional `DruidError` 搬迁的类型身份与可观察行为合同。

use std::error::Error;
use std::time::Duration;

use druid::core::error::DruidError as OldDruidError;
use druid_rdbc::compat::DruidError as TransitionalDruidError;
use druid_rdbc::error::{SqlException, SqlExceptionCause};
use druid_rdbc::types::RdbcString;

#[derive(Debug, PartialEq, Eq)]
struct ObservableError {
    display: String,
    class_name: String,
    sql_exception: Option<SqlException>,
    batch_update_counts: Option<Vec<i32>>,
    source_chain: Vec<String>,
}

type VariantCase = (
    &'static str,
    OldDruidError,
    usize,
    Option<i32>,
    Option<Vec<i32>>,
);

fn source_chain(error: &(dyn Error + 'static)) -> Vec<String> {
    let mut chain = Vec::new();
    let mut source = error.source();
    while let Some(current) = source {
        chain.push(current.to_string());
        source = current.source();
    }
    chain
}

fn observe(error: &TransitionalDruidError) -> ObservableError {
    ObservableError {
        display: error.to_string(),
        class_name: error.class_name().to_owned(),
        sql_exception: error.sql_exception().cloned(),
        batch_update_counts: error.batch_update_counts().map(<[i32]>::to_vec),
        source_chain: source_chain(error),
    }
}

fn assert_exhaustive_variant(error: &TransitionalDruidError, expected_name: &str) -> &'static str {
    let actual_name = match error {
        TransitionalDruidError::PoolClosed => "PoolClosed",
        TransitionalDruidError::DataSourceClosed { .. } => "DataSourceClosed",
        TransitionalDruidError::AcquireTimeout => "AcquireTimeout",
        TransitionalDruidError::GetConnectionTimeout { .. } => "GetConnectionTimeout",
        TransitionalDruidError::LoginTimeout => "LoginTimeout",
        TransitionalDruidError::PoolExhausted => "PoolExhausted",
        TransitionalDruidError::MaxWaitThreadCountExceeded { .. } => "MaxWaitThreadCountExceeded",
        TransitionalDruidError::ValidationFailed(_) => "ValidationFailed",
        TransitionalDruidError::ConnectionLeaked { .. } => "ConnectionLeaked",
        TransitionalDruidError::ConnectionDiscarded => "ConnectionDiscarded",
        TransitionalDruidError::DriverError(_) => "DriverError",
        TransitionalDruidError::SqlException(_) => "SqlException",
        TransitionalDruidError::BatchUpdateException { .. } => "BatchUpdateException",
        TransitionalDruidError::SqlParseError(_) => "SqlParseError",
        TransitionalDruidError::WallViolation(_) => "WallViolation",
        TransitionalDruidError::DataSourceNotFound(_) => "DataSourceNotFound",
        TransitionalDruidError::DataSourceDisabled => "DataSourceDisabled",
        TransitionalDruidError::ActiveConnectionsPreventRestart { .. } => {
            "ActiveConnectionsPreventRestart"
        }
        TransitionalDruidError::OnFatalError {
            active_count,
            max_active,
            last_error_time_millis,
            last_sql,
            cause,
        } => {
            assert_eq!(*active_count, 13);
            assert_eq!(*max_active, 12);
            assert_eq!(*last_error_time_millis, 1_725_678_901_234);
            assert_eq!(
                last_sql.as_ref().map(RdbcString::as_utf16),
                Some(&[0x0041, 0xD800, 0x0042][..])
            );
            assert!(cause.is_some());
            "OnFatalError"
        }
        TransitionalDruidError::DataSourceNotAvailable { .. } => "DataSourceNotAvailable",
        TransitionalDruidError::InvalidArgument(_) => "InvalidArgument",
        TransitionalDruidError::UnsupportedOperation { .. } => "UnsupportedOperation",
        TransitionalDruidError::NoDriverExtension { .. } => "NoDriverExtension",
        TransitionalDruidError::Other(_) => "Other",
    };
    assert_eq!(actual_name, expected_name);
    actual_name
}

#[test]
fn all_24_variants_keep_bidirectional_identity_and_observable_semantics() {
    let sql_exception = SqlException::new(
        1205,
        Some("40001".to_owned()),
        Some("deadlock victim".to_owned()),
    )
    .with_class_name("vendor.DeadlockException")
    .with_assignable_type("vendor.DatabaseException")
    .with_cause(SqlExceptionCause::ClassName("vendor.RootCause".to_owned()));
    let batch_sql_exception = SqlException::driver(1213, "batch retry");

    let cases: Vec<VariantCase> = vec![
        ("PoolClosed", OldDruidError::PoolClosed, 0, None, None),
        (
            "DataSourceClosed",
            OldDruidError::DataSourceClosed {
                close_time_millis: 1_725_678_901_234,
            },
            0,
            None,
            None,
        ),
        (
            "AcquireTimeout",
            OldDruidError::AcquireTimeout,
            0,
            None,
            None,
        ),
        (
            "GetConnectionTimeout",
            OldDruidError::GetConnectionTimeout {
                wait_millis: 5_000,
                active_count: 8,
                max_active: 8,
                creating_count: 1,
                create_elapsed_millis: Some(750),
                create_error_count: 2,
                running_sql: vec![(3, "select 1".to_owned())],
                cause: Some(Box::new(OldDruidError::DriverError(
                    "connect refused".to_owned(),
                ))),
            },
            1,
            None,
            None,
        ),
        ("LoginTimeout", OldDruidError::LoginTimeout, 0, None, None),
        ("PoolExhausted", OldDruidError::PoolExhausted, 0, None, None),
        (
            "MaxWaitThreadCountExceeded",
            OldDruidError::MaxWaitThreadCountExceeded {
                max: 16,
                current: 17,
            },
            0,
            None,
            None,
        ),
        (
            "ValidationFailed",
            OldDruidError::ValidationFailed("validation query failed".to_owned()),
            0,
            None,
            None,
        ),
        (
            "ConnectionLeaked",
            OldDruidError::ConnectionLeaked {
                id: 42,
                held_for: Duration::from_secs(30),
            },
            0,
            None,
            None,
        ),
        (
            "ConnectionDiscarded",
            OldDruidError::ConnectionDiscarded,
            0,
            None,
            None,
        ),
        (
            "DriverError",
            OldDruidError::DriverError("driver failed".to_owned()),
            0,
            None,
            None,
        ),
        (
            "SqlException",
            OldDruidError::SqlException(Box::new(sql_exception.clone())),
            0,
            Some(1205),
            None,
        ),
        (
            "BatchUpdateException",
            OldDruidError::BatchUpdateException {
                update_counts: vec![1, -2, -3],
                cause: Box::new(OldDruidError::SqlException(Box::new(
                    batch_sql_exception.clone(),
                ))),
            },
            1,
            Some(1213),
            Some(vec![1, -2, -3]),
        ),
        (
            "SqlParseError",
            OldDruidError::SqlParseError("unexpected token".to_owned()),
            0,
            None,
            None,
        ),
        (
            "WallViolation",
            OldDruidError::WallViolation("blocked statement".to_owned()),
            0,
            None,
            None,
        ),
        (
            "DataSourceNotFound",
            OldDruidError::DataSourceNotFound("orders".to_owned()),
            0,
            None,
            None,
        ),
        (
            "DataSourceDisabled",
            OldDruidError::DataSourceDisabled,
            0,
            None,
            None,
        ),
        (
            "ActiveConnectionsPreventRestart",
            OldDruidError::ActiveConnectionsPreventRestart { active_count: 5 },
            0,
            None,
            None,
        ),
        (
            "OnFatalError",
            OldDruidError::OnFatalError {
                active_count: 13,
                max_active: 12,
                last_error_time_millis: 1_725_678_901_234,
                last_sql: Some(RdbcString::from_utf16([0x0041, 0xD800, 0x0042])),
                cause: Some(Box::new(OldDruidError::DataSourceNotAvailable {
                    cause: Some(Box::new(OldDruidError::Other(
                        "nested fatal cause".to_owned(),
                    ))),
                })),
            },
            2,
            None,
            None,
        ),
        (
            "DataSourceNotAvailable",
            OldDruidError::DataSourceNotAvailable {
                cause: Some(Box::new(OldDruidError::Other(
                    "availability cause".to_owned(),
                ))),
            },
            1,
            None,
            None,
        ),
        (
            "InvalidArgument",
            OldDruidError::InvalidArgument("bad argument".to_owned()),
            0,
            None,
            None,
        ),
        (
            "UnsupportedOperation",
            OldDruidError::UnsupportedOperation { operation: "lob" },
            0,
            None,
            None,
        ),
        (
            "NoDriverExtension",
            OldDruidError::NoDriverExtension {
                db_type: "postgresql".to_owned(),
            },
            0,
            None,
            None,
        ),
        (
            "Other",
            OldDruidError::Other("other failure".to_owned()),
            0,
            None,
            None,
        ),
    ];

    assert_eq!(cases.len(), 24);
    for (expected_name, old, expected_source_depth, expected_sql_code, expected_counts) in cases {
        let old_before_move: TransitionalDruidError = old;
        let before = observe(&old_before_move);
        assert_eq!(
            before.sql_exception.as_ref().map(SqlException::error_code),
            expected_sql_code
        );
        assert_eq!(before.batch_update_counts, expected_counts);
        assert_eq!(before.source_chain.len(), expected_source_depth);

        let old_again: OldDruidError = old_before_move;
        let new_again: TransitionalDruidError = old_again;
        assert_eq!(observe(&new_again), before);
        assert_eq!(
            assert_exhaustive_variant(&new_again, expected_name),
            expected_name
        );
    }
}

#[test]
fn string_conversions_keep_other_variant_and_direct_type_identity() {
    let from_owned_old: OldDruidError = String::from("owned failure").into();
    let from_owned_new: TransitionalDruidError = from_owned_old;
    assert_eq!(from_owned_new.to_string(), "owned failure");
    assert!(matches!(
        from_owned_new,
        TransitionalDruidError::Other(ref message) if message == "owned failure"
    ));

    let from_borrowed_new: TransitionalDruidError = "borrowed failure".into();
    let from_borrowed_old: OldDruidError = from_borrowed_new;
    assert_eq!(from_borrowed_old.to_string(), "borrowed failure");
    assert!(matches!(
        from_borrowed_old,
        OldDruidError::Other(ref message) if message == "borrowed failure"
    ));
}
