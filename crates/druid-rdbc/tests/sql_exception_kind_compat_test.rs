use druid_rdbc::error::SqlExceptionKind;

#[test]
fn historical_sql_exception_variants_remain_exhaustively_matchable() {
    fn classify(kind: SqlExceptionKind) -> &'static str {
        use SqlExceptionKind::*;
        match kind {
            Transient => "transient",
            TransientConnection => "transient-connection",
            NonTransient => "non-transient",
            NonTransientConnection => "non-transient-connection",
            Recoverable => "recoverable",
            Data => "data",
            IntegrityConstraintViolation => "integrity",
            InvalidAuthorization => "authorization",
            Syntax => "syntax",
            TransactionRollback => "transaction",
            FeatureNotSupported => "unsupported",
            Timeout => "timeout",
            General => "general",
        }
    }

    assert_eq!(
        classify(SqlExceptionKind::InvalidAuthorizationSpec),
        "authorization"
    );
    assert_eq!(classify(SqlExceptionKind::Syntax), "syntax");
}
