// 具体 Java SQL 异常类型在可空 SQLState 之前确定异常分类。
use druid_rdbc::error::{SqlException, SqlExceptionKind};

#[test]
fn concrete_java_exception_class_precedes_nullable_or_conflicting_sql_state() {
    let cases = [
        (
            "java.sql.SQLTransientException",
            SqlExceptionKind::Transient,
        ),
        (
            "java.sql.SQLTransientConnectionException",
            SqlExceptionKind::TransientConnection,
        ),
        (
            "java.sql.SQLNonTransientException",
            SqlExceptionKind::NonTransient,
        ),
        (
            "java.sql.SQLNonTransientConnectionException",
            SqlExceptionKind::NonTransientConnection,
        ),
        (
            "java.sql.SQLRecoverableException",
            SqlExceptionKind::Recoverable,
        ),
        ("java.sql.SQLDataException", SqlExceptionKind::Data),
        (
            "java.sql.SQLIntegrityConstraintViolationException",
            SqlExceptionKind::IntegrityConstraintViolation,
        ),
        (
            "java.sql.SQLInvalidAuthorizationSpecException",
            SqlExceptionKind::InvalidAuthorization,
        ),
        ("java.sql.SQLSyntaxErrorException", SqlExceptionKind::Syntax),
        (
            "java.sql.SQLTransactionRollbackException",
            SqlExceptionKind::TransactionRollback,
        ),
        (
            "java.sql.SQLFeatureNotSupportedException",
            SqlExceptionKind::FeatureNotSupported,
        ),
        ("java.sql.SQLTimeoutException", SqlExceptionKind::Timeout),
    ];
    for (class_name, expected) in cases {
        for state in [None, Some("42S02")] {
            let exception =
                SqlException::new(17, state.map(str::to_owned), Some("original".to_owned()))
                    .with_class_name(class_name);
            assert_eq!(exception.kind(), expected, "{class_name}, {state:?}");
            assert_eq!(exception.error_code(), 17);
            assert_eq!(exception.message(), Some("original"));
            assert_eq!(exception.sql_state(), state);
            assert_eq!(exception.class_name(), class_name);
        }
    }
}

#[test]
fn vendor_exceptions_retain_sql_state_and_recoverable_fallbacks() {
    let cases = [
        (Some("08001"), SqlExceptionKind::NonTransientConnection),
        (Some("22001"), SqlExceptionKind::Data),
        (
            Some("23000"),
            SqlExceptionKind::IntegrityConstraintViolation,
        ),
        (Some("28000"), SqlExceptionKind::InvalidAuthorization),
        (Some("40001"), SqlExceptionKind::TransactionRollback),
        (Some("42000"), SqlExceptionKind::Syntax),
        (Some("0A000"), SqlExceptionKind::FeatureNotSupported),
        (Some("HYT00"), SqlExceptionKind::Timeout),
        (Some("HY000"), SqlExceptionKind::General),
        (Some(""), SqlExceptionKind::General),
        (Some("2"), SqlExceptionKind::General),
        (Some("界"), SqlExceptionKind::General),
        (None, SqlExceptionKind::General),
    ];
    for (state, expected) in cases {
        let exception =
            SqlException::new(9, state.map(str::to_owned), None).with_class_name("vendor.Error");
        assert_eq!(exception.kind(), expected, "{state:?}");
        assert_eq!(
            exception.clone().recoverable().kind(),
            SqlExceptionKind::Recoverable
        );
    }
}
