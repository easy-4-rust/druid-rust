//! JDBC ClientInfoStatus 与九个 SQLClientInfoException 构造器的标准合同。
use druid_rdbc::connection::{ClientInfoStatus, ClientInfoStatusNameError};
use druid_rdbc::error::{
    ProductErrorBoundary, RdbcError, SqlClientInfoException, SqlException, SqlExceptionCause,
};
use std::collections::HashMap;
use std::error::Error;

fn failures() -> HashMap<Option<String>, Option<ClientInfoStatus>> {
    HashMap::from([
        (
            Some("User".to_owned()),
            Some(ClientInfoStatus::ReasonValueInvalid),
        ),
        (
            Some("user".to_owned()),
            Some(ClientInfoStatus::ReasonUnknownProperty),
        ),
        (
            Some(String::new()),
            Some(ClientInfoStatus::ReasonValueTruncated),
        ),
        (None, Some(ClientInfoStatus::ReasonUnknown)),
        (Some("unspecified".to_owned()), None),
    ])
}
#[test]
fn status_values_names_order_and_exact_parsing_match_java() {
    let expected = [
        "REASON_UNKNOWN",
        "REASON_UNKNOWN_PROPERTY",
        "REASON_VALUE_INVALID",
        "REASON_VALUE_TRUNCATED",
    ];
    let mut values = ClientInfoStatus::values();
    for (index, value) in values.into_iter().enumerate() {
        assert_eq!(value.ordinal(), i32::try_from(index).unwrap());
        assert_eq!(value.name(), expected[index]);
        assert_eq!(value.to_string(), expected[index]);
        assert_eq!(
            ClientInfoStatus::value_of(Some(expected[index])).unwrap(),
            value
        );
        if index > 0 {
            assert!(value > values[index - 1]);
        }
    }
    values[0] = ClientInfoStatus::ReasonValueInvalid;
    assert_ne!(values[0], ClientInfoStatus::values()[0]);
    assert_eq!(
        ClientInfoStatus::values()[0],
        ClientInfoStatus::ReasonUnknown
    );
    assert_eq!(
        ClientInfoStatus::value_of(None),
        Err(ClientInfoStatusNameError::NullName)
    );
    assert_eq!(
        ClientInfoStatusNameError::NullName.class_name(),
        "java.lang.NullPointerException"
    );
    for name in [
        "",
        "reason_unknown",
        " REASON_UNKNOWN",
        "REASON_UNKNOWN ",
        "vendor:status",
    ] {
        let error = ClientInfoStatus::value_of(Some(name)).unwrap_err();
        assert_eq!(error.class_name(), "java.lang.IllegalArgumentException");
        assert_eq!(
            error,
            ClientInfoStatusNameError::UnknownName(name.to_owned())
        );
    }
}
#[test]
fn default_null_empty_maps_and_nullable_entries_remain_distinct() {
    let default = SqlClientInfoException::new();
    assert_eq!(default, SqlClientInfoException::default());
    assert_eq!(default.message(), None);
    assert_eq!(default.sql_state(), None);
    assert_eq!(default.error_code(), 0);
    assert!(default.failed_properties().is_none());
    assert!(default.cause().is_none());
    let empty = SqlClientInfoException::with_failed_properties(Some(HashMap::new()));
    assert!(empty.failed_properties().unwrap().is_empty());
    assert_ne!(default, empty);
    let input = failures();
    let mut error = SqlClientInfoException::with_failed_properties(Some(input.clone()));
    assert_eq!(error.failed_properties(), Some(&input));
    assert_eq!(
        error.failed_properties().unwrap()[&None],
        Some(ClientInfoStatus::ReasonUnknown)
    );
    assert_eq!(
        error.failed_properties().unwrap()[&Some("unspecified".to_owned())],
        None
    );
    error.failed_properties_mut().unwrap().insert(
        Some("added".to_owned()),
        Some(ClientInfoStatus::ReasonUnknown),
    );
    assert_eq!(error.failed_properties().unwrap().len(), 6);
    assert_eq!(input.len(), 5);
}
#[test]
fn all_nine_constructors_preserve_diagnostics_explicit_reason_and_cause() {
    let map = || Some(failures());
    let cause = RdbcError::DriverError("test-cause".to_owned());
    let cases = [
        (SqlClientInfoException::new(), None, None, 0, false, false),
        (
            SqlClientInfoException::with_failed_properties(map()),
            None,
            None,
            0,
            false,
            true,
        ),
        (
            SqlClientInfoException::with_reason(Some("reason".to_owned()), map()),
            Some("reason"),
            None,
            0,
            false,
            true,
        ),
        (
            SqlClientInfoException::with_reason_state(
                Some("reason".to_owned()),
                Some("01S00".to_owned()),
                map(),
            ),
            Some("reason"),
            Some("01S00"),
            0,
            false,
            true,
        ),
        (
            SqlClientInfoException::with_details(
                Some("reason".to_owned()),
                Some("01S00".to_owned()),
                77,
                map(),
            ),
            Some("reason"),
            Some("01S00"),
            77,
            false,
            true,
        ),
        (
            SqlClientInfoException::with_cause(map(), Some(cause.clone())),
            Some("driver error: test-cause"),
            None,
            0,
            true,
            true,
        ),
        (
            SqlClientInfoException::with_reason_cause(None, map(), Some(cause.clone())),
            None,
            None,
            0,
            true,
            true,
        ),
        (
            SqlClientInfoException::with_reason_state_cause(
                Some("reason".to_owned()),
                Some("01S00".to_owned()),
                map(),
                Some(cause.clone()),
            ),
            Some("reason"),
            Some("01S00"),
            0,
            true,
            true,
        ),
        (
            SqlClientInfoException::with_details_cause(
                Some("reason".to_owned()),
                Some("01S00".to_owned()),
                77,
                map(),
                Some(cause.clone()),
            ),
            Some("reason"),
            Some("01S00"),
            77,
            true,
            true,
        ),
    ];
    for (mut error, reason, state, code, has_cause, has_map) in cases {
        assert_eq!(error.message(), reason);
        assert_eq!(error.sql_state(), state);
        assert_eq!(error.error_code(), code);
        assert_eq!(error.class_name(), "java.sql.SQLClientInfoException");
        assert!(error.is_instance_of("java.sql.SQLException"));
        assert!(error.is_instance_of("java.sql.SQLClientInfoException"));
        assert!(!error.is_instance_of("java.sql.SQLDataException"));
        assert_eq!(error.failed_properties().is_some(), has_map);
        if has_map {
            assert_eq!(error.failed_properties(), Some(&failures()));
        }
        assert_eq!(error.cause(), has_cause.then_some(&cause));
        assert_eq!(
            error
                .source()
                .and_then(|value| value.downcast_ref::<RdbcError>()),
            error.cause()
        );
        error.set_next_exception(SqlException::driver(81, "next"));
        assert_eq!(error.next_exception().unwrap().error_code(), 81);
    }
    let null_cause = SqlClientInfoException::with_cause(None, None);
    assert_eq!(null_cause.message(), None);
    assert!(null_cause.cause().is_none());
    assert!(null_cause.failed_properties().is_none());
}
#[test]
fn canonical_error_bridge_retains_typed_payload_sql_chain_and_original_source() {
    let mut sql = SqlException::new(77, Some("22023".to_owned()), Some("rejected".to_owned()))
        .with_class_name("java.sql.SQLDataException")
        .with_cause(SqlExceptionCause::SocketTimeout);
    sql.set_next_exception(SqlException::driver(81, "next"));
    let original = RdbcError::SqlException(Box::new(sql));
    let error = original.clone().with_client_info_failure(failures());
    assert_eq!(error.class_name(), "java.sql.SQLClientInfoException");
    let outer = error.sql_exception().unwrap();
    assert_eq!(outer.class_name(), "java.sql.SQLClientInfoException");
    assert!(!outer.is_instance_of("java.sql.SQLDataException"));
    assert_eq!(outer.sql_state(), Some("22023"));
    assert_eq!(outer.error_code(), 77);
    assert_eq!(outer.next_exception().unwrap().error_code(), 81);
    assert_eq!(
        outer.causes(),
        &[
            SqlExceptionCause::ClassName("java.sql.SQLDataException".to_owned()),
            SqlExceptionCause::SocketTimeout
        ]
    );
    assert_eq!(error.failed_client_info_properties(), Some(&failures()));
    let payload = error
        .source()
        .unwrap()
        .downcast_ref::<SqlClientInfoException>()
        .unwrap();
    assert_eq!(payload.cause(), Some(&original));
    assert_eq!(
        payload.source().unwrap().downcast_ref::<RdbcError>(),
        Some(&original)
    );
}
#[test]
fn transparent_product_boundary_preserves_complete_or_null_failure_maps() {
    for failed in [None, Some(failures())] {
        let raw: RdbcError = SqlClientInfoException::with_details(
            Some("driver".to_owned()),
            Some("01S00".to_owned()),
            7,
            failed.clone(),
        )
        .into();
        let wrapped = RdbcError::ProductBoundary(ProductErrorBoundary::new(
            "product.ClientInfoBridge",
            "wrapped",
            Some(raw),
        ));
        let normalized = wrapped.clone().with_client_info_failure(HashMap::from([(
            Some("success".to_owned()),
            Some(ClientInfoStatus::ReasonUnknown),
        )]));
        assert_eq!(normalized.class_name(), "java.sql.SQLClientInfoException");
        assert_eq!(normalized.failed_client_info_properties(), failed.as_ref());
        assert_eq!(normalized.sql_exception().unwrap().error_code(), 7);
        let RdbcError::ClientInfoException { exception } = normalized else {
            panic!("standard payload")
        };
        assert_eq!(exception.as_ref().cause(), Some(&wrapped));
    }
}
#[test]
fn default_and_empty_exception_payloads_round_trip_without_null_collapse() {
    for payload in [
        SqlClientInfoException::new(),
        SqlClientInfoException::with_failed_properties(Some(HashMap::new())),
    ] {
        let expected = payload.clone();
        let error: RdbcError = payload.into();
        let unchanged = error.clone().with_client_info_failure(failures());
        assert_eq!(error, unchanged);
        let RdbcError::ClientInfoException { exception } = error else {
            panic!("standard payload")
        };
        assert_eq!(*exception, expected);
    }
}
