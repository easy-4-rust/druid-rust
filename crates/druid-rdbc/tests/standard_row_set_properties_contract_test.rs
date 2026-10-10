//! 完整 RowSet 的属性状态合同；执行、事件及写回需由后续标准实现另行验证。
use druid_rdbc::connection::TransactionIsolation;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::RowSetProperties;
use druid_rdbc::statement::{ResultSetConcurrency, ResultSetType};
use druid_rdbc::types::{RdbcTargetType, RdbcTypeMap};
use std::sync::{Arc, Mutex};

#[test]
fn defaults_preserve_nullable_properties_and_unlimited_limits() {
    let p = RowSetProperties::new();
    assert_eq!(p.url(), None);
    assert_eq!(p.data_source_name(), None);
    assert_eq!(p.username(), None);
    assert_eq!(p.password(), None);
    assert_eq!(p.command(), None);
    assert!(p.type_map().lock().unwrap().is_empty());
    assert_eq!(p.max_field_size(), 0);
    assert_eq!(p.max_rows(), 0);
    assert_eq!(p.query_timeout(), 0);
    assert!(p.escape_processing());
    assert!(p.is_read_only());
    assert_eq!(p.result_set_type(), ResultSetType::ForwardOnly);
    assert_eq!(p.concurrency(), ResultSetConcurrency::ReadOnly);
}

#[test]
fn source_precedence_uses_last_nonnull_assignment_not_lexical_order() {
    let mut p = RowSetProperties::new();
    p.set_url(Some(" jdbc:test:数据库 ")).unwrap();
    assert_eq!(p.preferred_connection_url(), Some(" jdbc:test:数据库 "));
    p.set_data_source_name(Some(" java:comp/env/数据源 "))
        .unwrap();
    assert_eq!(p.url(), Some(" jdbc:test:数据库 "));
    assert_eq!(p.preferred_connection_url(), None);
    assert_eq!(
        p.preferred_data_source_name(),
        Some(" java:comp/env/数据源 ")
    );
    p.set_url(Some("jdbc:test:second")).unwrap();
    assert_eq!(p.preferred_connection_url(), Some("jdbc:test:second"));
    assert_eq!(p.preferred_data_source_name(), None);
    p.set_data_source_name(None).unwrap();
    assert_eq!(p.preferred_connection_url(), Some("jdbc:test:second"));
    p.set_url(None).unwrap();
    assert_eq!(p.preferred_connection_url(), None);
    assert_eq!(p.preferred_data_source_name(), None);
    p.set_data_source_name(Some("")).unwrap();
    assert_eq!(p.preferred_data_source_name(), Some(""));
    p.set_url(Some("later")).unwrap();
    p.set_url(None).unwrap();
    assert_eq!(p.preferred_data_source_name(), Some(""));
}

#[test]
fn text_properties_round_trip_null_empty_unicode_without_debug_secrets() {
    let mut p = RowSetProperties::new();
    p.set_username(Some(" username-secret-用户 ")).unwrap();
    p.set_password(Some("password-secret-密码")).unwrap();
    p.set_command(Some("select 'command-secret-业务'")).unwrap();
    p.set_url(Some("jdbc:test:url-secret")).unwrap();
    p.set_data_source_name(Some("data-source-secret")).unwrap();
    assert_eq!(p.username(), Some(" username-secret-用户 "));
    assert_eq!(p.password(), Some("password-secret-密码"));
    assert_eq!(p.command(), Some("select 'command-secret-业务'"));
    let debug = format!("{p:?}");
    for secret in [
        "username-secret",
        "password-secret",
        "command-secret",
        "url-secret",
        "data-source-secret",
    ] {
        assert!(!debug.contains(secret), "Debug disclosed {secret}");
    }
    p.set_username(Some("")).unwrap();
    p.set_password(Some("")).unwrap();
    p.set_command(Some("")).unwrap();
    assert_eq!(p.username(), Some(""));
    assert_eq!(p.password(), Some(""));
    assert_eq!(p.command(), Some(""));
    p.set_username(None).unwrap();
    p.set_password(None).unwrap();
    p.set_command(None).unwrap();
    assert_eq!(p.username(), None);
    assert_eq!(p.password(), None);
    assert_eq!(p.command(), None);
}

#[test]
fn numeric_limits_reject_negative_atomically_and_preserve_full_i32_range() {
    let mut p = RowSetProperties::new();
    p.set_max_field_size(257).unwrap();
    p.set_max_rows(17).unwrap();
    p.set_query_timeout(23).unwrap();
    for invalid in [-1, i32::MIN] {
        assert!(matches!(
            p.set_max_field_size(invalid),
            Err(RdbcError::SqlException(_))
        ));
        assert!(matches!(
            p.set_max_rows(invalid),
            Err(RdbcError::SqlException(_))
        ));
        assert!(matches!(
            p.set_query_timeout(invalid),
            Err(RdbcError::SqlException(_))
        ));
        assert_eq!(
            (p.max_field_size(), p.max_rows(), p.query_timeout()),
            (257, 17, 23)
        );
    }
    for valid in [0, i32::MAX] {
        p.set_max_field_size(valid).unwrap();
        p.set_max_rows(valid).unwrap();
        p.set_query_timeout(valid).unwrap();
        assert_eq!(
            (p.max_field_size(), p.max_rows(), p.query_timeout()),
            (valid, valid, valid)
        );
    }
}

#[test]
fn isolation_cursor_and_concurrency_validate_without_claiming_provider_support() {
    let mut p = RowSetProperties::new();
    for isolation in [
        TransactionIsolation::READ_UNCOMMITTED,
        TransactionIsolation::READ_COMMITTED,
        TransactionIsolation::REPEATABLE_READ,
        TransactionIsolation::SERIALIZABLE,
    ] {
        p.set_transaction_isolation(isolation).unwrap();
        assert_eq!(p.transaction_isolation(), isolation);
    }
    for invalid in [0, -1, 3, i32::MAX] {
        assert!(matches!(
            p.set_transaction_isolation(TransactionIsolation::from_jdbc(invalid)),
            Err(RdbcError::SqlException(_))
        ));
        assert_eq!(
            p.transaction_isolation(),
            TransactionIsolation::SERIALIZABLE
        );
    }
    for (code, expected) in [
        (1003, ResultSetType::ForwardOnly),
        (1004, ResultSetType::ScrollInsensitive),
        (1005, ResultSetType::ScrollSensitive),
    ] {
        p.set_type(code).unwrap();
        assert_eq!(p.result_set_type(), expected);
    }
    for invalid in [0, -1, 1002, 1006, i32::MAX] {
        assert!(matches!(
            p.set_type(invalid),
            Err(RdbcError::SqlException(_))
        ));
        assert_eq!(p.result_set_type(), ResultSetType::ScrollSensitive);
    }
    for (code, expected) in [
        (1007, ResultSetConcurrency::ReadOnly),
        (1008, ResultSetConcurrency::Updatable),
    ] {
        p.set_concurrency(code).unwrap();
        assert_eq!(p.concurrency(), expected);
    }
    assert!(p.set_concurrency(1009).is_err());
    assert_eq!(p.concurrency(), ResultSetConcurrency::Updatable);
    p.set_read_only(false).unwrap();
    p.set_escape_processing(false).unwrap();
    assert!(!p.is_read_only());
    assert!(!p.escape_processing());
    // 模式保存并不宣称驱动实际支持更新或敏感游标，执行必须另行校验能力。
}

#[test]
fn installed_type_map_retains_shared_identity_and_replacement_isolated_from_clone() {
    let mut p = RowSetProperties::new();
    let map = Arc::new(Mutex::new(RdbcTypeMap::new()));
    p.set_type_map(Arc::clone(&map)).unwrap();
    assert!(Arc::ptr_eq(&map, &p.type_map()));
    map.lock()
        .unwrap()
        .insert("schema.类型", RdbcTargetType::Custom("AppType".into()));
    assert_eq!(
        p.type_map().lock().unwrap().get("schema.类型"),
        Some(&RdbcTargetType::Custom("AppType".into()))
    );
    let snapshot = p.clone();
    assert!(Arc::ptr_eq(&p.type_map(), &snapshot.type_map()));
    let replacement = Arc::new(Mutex::new(RdbcTypeMap::new()));
    p.set_type_map(Arc::clone(&replacement)).unwrap();
    assert!(Arc::ptr_eq(&replacement, &p.type_map()));
    assert!(Arc::ptr_eq(&map, &snapshot.type_map()));
}
