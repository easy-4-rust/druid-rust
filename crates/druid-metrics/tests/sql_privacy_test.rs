use serde_json::json;

use druid_metrics::config::SqlTextPolicy;
use druid_metrics::error::MetricsError;
use druid_metrics::sanitizer::sanitize_payload;

#[test]
fn fingerprint_only_strips_raw_sql_but_keeps_fingerprint() {
    let payload = json!({
        "fingerprint": "SELECT * FROM users WHERE id = ?",
        "parameterized_sql": "SELECT * FROM users WHERE id = ?",
        "raw_sql": "SELECT * FROM users WHERE id = 42",
        "exec_count": 10,
        "exec_time_millis": 150
    });

    let result = sanitize_payload(&payload, SqlTextPolicy::FingerprintOnly).unwrap();

    // Fingerprint and parameterized_sql should be preserved
    assert_eq!(
        result.get("fingerprint").unwrap().as_str().unwrap(),
        "SELECT * FROM users WHERE id = ?"
    );
    assert_eq!(
        result.get("parameterized_sql").unwrap().as_str().unwrap(),
        "SELECT * FROM users WHERE id = ?"
    );

    // Raw SQL should be stripped
    assert!(result.get("raw_sql").is_none());

    // Non-SQL fields preserved
    assert_eq!(result.get("exec_count").unwrap().as_i64().unwrap(), 10);
    assert_eq!(
        result.get("exec_time_millis").unwrap().as_i64().unwrap(),
        150
    );
}

#[test]
fn disabled_policy_strips_all_sql_fields() {
    let payload = json!({
        "fingerprint": "SELECT * FROM users WHERE id = ?",
        "parameterized_sql": "SELECT * FROM users WHERE id = ?",
        "raw_sql": "SELECT * FROM users WHERE id = 42",
        "exec_count": 10
    });

    let result = sanitize_payload(&payload, SqlTextPolicy::Disabled).unwrap();

    assert!(result.get("fingerprint").is_none());
    assert!(result.get("parameterized_sql").is_none());
    assert!(result.get("raw_sql").is_none());
    assert_eq!(result.get("exec_count").unwrap().as_i64().unwrap(), 10);
}

#[test]
fn raw_without_parameters_strips_bind_values() {
    let payload = json!({
        "sql": "SELECT * FROM users WHERE id = ?",
        "bind_values": [42, "alice"],
        "exec_count": 5
    });

    let result = sanitize_payload(&payload, SqlTextPolicy::RawWithoutParameters).unwrap();

    // bind_values should be stripped
    assert!(result.get("bind_values").is_none());
    // sql should be kept (RawWithoutParameters keeps raw SQL)
    assert!(result.get("sql").is_some());
    assert_eq!(result.get("exec_count").unwrap().as_i64().unwrap(), 5);
}

#[test]
fn sensitive_field_password_is_rejected() {
    let payload = json!({
        "password": "hunter2",
        "username": "admin"
    });

    let err = sanitize_payload(&payload, SqlTextPolicy::FingerprintOnly).unwrap_err();
    match err {
        MetricsError::SensitiveField { field } => {
            assert_eq!(field, "password");
        }
        other => panic!("expected SensitiveField, got: {other}"),
    }
}

#[test]
fn sensitive_field_token_is_rejected() {
    let payload = json!({
        "token": "jwt-abc-123"
    });

    let err = sanitize_payload(&payload, SqlTextPolicy::FingerprintOnly).unwrap_err();
    assert!(matches!(err, MetricsError::SensitiveField { .. }));
}

#[test]
fn sensitive_field_in_nested_object_is_rejected() {
    let payload = json!({
        "connection": {
            "host": "localhost",
            "password": "secret123"
        }
    });

    let err = sanitize_payload(&payload, SqlTextPolicy::FingerprintOnly).unwrap_err();
    assert!(matches!(err, MetricsError::SensitiveField { .. }));
}

#[test]
fn sensitive_field_in_array_is_rejected() {
    let payload = json!([
        { "name": "db1" },
        { "token": "secret-token" }
    ]);

    let err = sanitize_payload(&payload, SqlTextPolicy::FingerprintOnly).unwrap_err();
    assert!(matches!(err, MetricsError::SensitiveField { .. }));
}

#[test]
fn bind_parameters_variant_is_stripped() {
    let payload = json!({
        "bindParameters": [1, 2, 3],
        "exec_count": 5
    });

    let result = sanitize_payload(&payload, SqlTextPolicy::FingerprintOnly).unwrap();
    // bindParameters should be stripped silently (not rejected)
    assert!(result.get("bindParameters").is_none());
    assert_eq!(result.get("exec_count").unwrap().as_i64().unwrap(), 5);
}

#[test]
fn clean_payload_passes_all_policies() {
    let payload = json!({
        "fingerprint": "SELECT count(*) FROM orders",
        "parameterized_sql": "SELECT count(*) FROM orders",
        "exec_count": 100,
        "exec_time_millis": 5000
    });

    // Should pass all three policies
    assert!(sanitize_payload(&payload, SqlTextPolicy::Disabled).is_ok());
    assert!(sanitize_payload(&payload, SqlTextPolicy::FingerprintOnly).is_ok());
    assert!(sanitize_payload(&payload, SqlTextPolicy::RawWithoutParameters).is_ok());
}

#[test]
fn fingerprint_only_with_formatted_sql_field() {
    let payload = json!({
        "fingerprint": "INSERT INTO logs VALUES (?, ?)",
        "formatted_sql": "INSERT INTO logs\nVALUES (?, ?)",
        "raw_sql": "INSERT INTO logs VALUES (1, 'test')",
        "exec_count": 50
    });

    let result = sanitize_payload(&payload, SqlTextPolicy::FingerprintOnly).unwrap();

    assert!(result.get("fingerprint").is_some());
    assert!(result.get("formatted_sql").is_none()); // stripped
    assert!(result.get("raw_sql").is_none()); // stripped
}
