use std::collections::HashMap;
use std::error::Error;

use druid_rdbc::compat::{DataTruncation, SqlClientInfoException};
use druid_rdbc::error::{
    BatchUpdateCounts, RdbcError, RdbcErrorKind, SqlException, SqlExceptionCause, SqlWarning,
};
use druid_rdbc::statement::Statement;

struct LargeThenFailBatchStatement {
    batch_size: usize,
}

druid_rdbc::impl_standard_wrapper!(LargeThenFailBatchStatement => Statement);

#[async_trait::async_trait]
impl Statement for LargeThenFailBatchStatement {
    fn add_batch(&mut self, _sql: String) -> Result<(), RdbcError> {
        self.batch_size += 1;
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        let large = i64::from(i32::MAX) + 7;
        if self.batch_size > 1 {
            return Err(RdbcError::BatchUpdateException {
                update_counts: BatchUpdateCounts::from_large(vec![large]),
                cause: Box::new(RdbcError::DriverError(
                    "second batch item failed".to_owned(),
                )),
            });
        }
        Ok(vec![large])
    }
}

#[test]
fn batch_update_counts_preserve_large_java_cast_and_checked_views() {
    let counts = BatchUpdateCounts::from_large(vec![1, i64::from(i32::MAX) + 7, -2, -3]);

    assert_eq!(counts.large()[1], i64::from(i32::MAX) + 7);
    assert_eq!(counts.jdbc_i32()[1], (i64::from(i32::MAX) + 7) as i32);
    assert!(counts.try_i32().is_err());
    assert_eq!(counts.jdbc_i32(), vec![1, i32::MIN + 6, -2, -3]);
}

#[test]
fn sql_exception_preserves_state_vendor_cause_next_and_rust_source() {
    let mut exception = SqlException::new(
        1205,
        Some("40001".to_owned()),
        Some("deadlock victim".to_owned()),
    )
    .with_class_name("vendor.DeadlockException")
    .with_assignable_type("vendor.DatabaseException")
    .recoverable()
    .with_cause(SqlExceptionCause::SocketTimeout)
    .with_cause(SqlExceptionCause::ClassName("vendor.RootCause".to_owned()));
    exception.set_next_exception(
        SqlException::driver(1213, "retry transaction").with_sql_state("40002"),
    );

    let error = RdbcError::SqlException(Box::new(exception));
    let observed = error.sql_exception().expect("SQL payload must be retained");
    assert_eq!(observed.error_code(), 1205);
    assert_eq!(observed.sql_state(), Some("40001"));
    assert_eq!(observed.message(), Some("deadlock victim"));
    assert_eq!(observed.class_name(), "vendor.DeadlockException");
    assert_eq!(
        observed.causes(),
        &[
            SqlExceptionCause::SocketTimeout,
            SqlExceptionCause::ClassName("vendor.RootCause".to_owned()),
        ]
    );
    assert_eq!(
        observed.next_exception().and_then(SqlException::sql_state),
        Some("40002")
    );
    assert_eq!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<SqlException>()),
        Some(observed)
    );
}

#[test]
fn warning_client_info_and_data_truncation_payloads_are_lossless() {
    let mut warning = SqlWarning::new("first", Some("01000".to_owned()), 7);
    warning.set_next_warning(SqlWarning::new("second", Some("01001".to_owned()), 8));
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 7);
    assert_eq!(
        warning.next_warning().map(SqlWarning::message),
        Some("second")
    );

    let mut failed_properties = HashMap::new();
    failed_properties.insert(
        "ApplicationName".to_owned(),
        "REASON_VALUE_INVALID".to_owned(),
    );
    failed_properties.insert(
        "ClientUser".to_owned(),
        "REASON_UNKNOWN_PROPERTY".to_owned(),
    );
    let client_info = SqlClientInfoException {
        exception: SqlException::new(
            0,
            Some("01S00".to_owned()),
            Some("client info rejected".to_owned()),
        ),
        failed_properties: failed_properties.clone(),
    };
    assert_eq!(client_info.exception.sql_state(), Some("01S00"));
    assert_eq!(client_info.failed_properties, failed_properties);

    let truncation = DataTruncation {
        index: 3,
        parameter: true,
        read: false,
        data_size: Some(64),
        transfer_size: Some(32),
    };
    assert_eq!(truncation.index, 3);
    assert!(truncation.parameter);
    assert!(!truncation.read);
    assert_eq!(truncation.data_size, Some(64));
    assert_eq!(truncation.transfer_size, Some(32));
}

#[test]
fn sql_feature_and_default_unsupported_taxonomies_remain_distinct() {
    let sql_feature = RdbcError::feature_not_supported("Connection#createArrayOf");
    assert_eq!(sql_feature.kind(), RdbcErrorKind::SqlFeatureNotSupported);
    assert_eq!(
        sql_feature.class_name(),
        "java.sql.SQLFeatureNotSupportedException"
    );
    assert_eq!(
        sql_feature
            .sql_exception()
            .and_then(SqlException::sql_state),
        Some("0A000")
    );

    let default_unsupported = RdbcError::UnsupportedOperation {
        operation: "Connection#beginRequest",
    };
    assert_eq!(
        default_unsupported.kind(),
        RdbcErrorKind::UnsupportedOperation
    );
    assert_eq!(
        default_unsupported.class_name(),
        "java.lang.UnsupportedOperationException"
    );
    assert!(default_unsupported.sql_exception().is_none());
}

#[test]
fn batch_error_preserves_i64_counts_and_typed_cause_chain() {
    let counts = BatchUpdateCounts::from_large(vec![1, i64::from(i32::MAX) + 7, -3]);
    let error = RdbcError::BatchUpdateException {
        update_counts: counts,
        cause: Box::new(RdbcError::DriverError("batch driver failed".to_owned())),
    };

    assert_eq!(
        error.batch_update_counts().map(BatchUpdateCounts::large),
        Some(&[1, i64::from(i32::MAX) + 7, -3][..])
    );
    assert!(matches!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<RdbcError>()),
        Some(RdbcError::DriverError(message)) if message == "batch driver failed"
    ));
}

#[tokio::test]
async fn canonical_batch_path_preserves_large_failure_counts_and_java_success_view() {
    let large = i64::from(i32::MAX) + 7;
    let mut failing = LargeThenFailBatchStatement { batch_size: 0 };
    failing
        .add_batch("large".to_owned())
        .expect("first batch item must be accepted");
    failing
        .add_batch("fail".to_owned())
        .expect("second batch item must be accepted");
    let error = failing
        .execute_batch()
        .await
        .expect_err("second batch item must fail");
    let counts = error
        .batch_update_counts()
        .expect("first large update count must survive the failure path");
    assert_eq!(counts.large(), &[large]);
    assert_eq!(counts.jdbc_i32(), vec![large as i32]);
    assert!(counts.try_i32().is_err());

    let mut successful = LargeThenFailBatchStatement { batch_size: 0 };
    successful
        .add_batch("large".to_owned())
        .expect("single batch item must be accepted");
    assert_eq!(successful.execute_batch().await, Ok(vec![large]));
}
