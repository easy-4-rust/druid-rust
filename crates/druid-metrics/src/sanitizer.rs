use serde_json::Value;

use crate::config::SqlTextPolicy;
use crate::error::MetricsError;

/// Genuinely sensitive field names that must NEVER appear in any export.
/// These are always rejected regardless of policy.
const ALWAYS_SENSITIVE_FIELDS: &[&str] = &["password", "token", "secret"];

/// Bind-value field names that policies strip (not reject).
const BIND_VALUE_FIELDS: &[&str] = &[
    "bind_values",
    "bindparameters",
    "bind_parameters",
    "args",
    "arguments",
];

/// Sanitize a JSON payload according to the SQL text policy.
///
/// Recursively walks the value and:
/// 1. Removes SQL text fields that violate the policy.
/// 2. Removes bind-value fields for policies that don't include them.
/// 3. Returns `Err(MetricsError::SensitiveField)` if passwords, tokens,
///    or secrets are detected.
pub fn sanitize_payload(value: &Value, policy: SqlTextPolicy) -> Result<Value, MetricsError> {
    match policy {
        SqlTextPolicy::Disabled => sanitize_disabled(value),
        SqlTextPolicy::FingerprintOnly => sanitize_fingerprint_only(value),
        SqlTextPolicy::RawWithoutParameters => sanitize_raw_no_params(value),
    }
}

/// Disabled policy: strip all SQL text fields and bind values.
fn sanitize_disabled(value: &Value) -> Result<Value, MetricsError> {
    check_always_sensitive(value)?;
    match value {
        Value::Object(map) => {
            let mut cleaned = map.clone();
            cleaned.remove("sql");
            cleaned.remove("raw_sql");
            cleaned.remove("rawSql");
            cleaned.remove("formatted_sql");
            cleaned.remove("formattedSql");
            cleaned.remove("fingerprint");
            cleaned.remove("parameterized_sql");
            cleaned.remove("parameterizedSql");
            strip_bind_value_fields(&mut cleaned);
            for v in cleaned.values_mut() {
                *v = sanitize_disabled(v)?;
            }
            Ok(Value::Object(cleaned))
        }
        Value::Array(arr) => {
            let cleaned: Result<Vec<Value>, MetricsError> =
                arr.iter().map(sanitize_disabled).collect();
            Ok(Value::Array(cleaned?))
        }
        other => Ok(other.clone()),
    }
}

/// `FingerprintOnly` policy: keep fingerprint and parameterized SQL,
/// strip raw SQL and bind values.
fn sanitize_fingerprint_only(value: &Value) -> Result<Value, MetricsError> {
    check_always_sensitive(value)?;
    if let Value::Object(map) = value {
        let mut cleaned = map.clone();
        // Keep: fingerprint, parameterized_sql
        // Remove: raw_sql, sql, formatted_sql
        cleaned.remove("raw_sql");
        cleaned.remove("rawSql");
        cleaned.remove("formatted_sql");
        cleaned.remove("formattedSql");
        cleaned.remove("sql");
        strip_bind_value_fields(&mut cleaned);
        for v in cleaned.values_mut() {
            *v = sanitize_fingerprint_only(v)?;
        }
        Ok(Value::Object(cleaned))
    } else if let Value::Array(arr) = value {
        let cleaned: Result<Vec<Value>, MetricsError> =
            arr.iter().map(sanitize_fingerprint_only).collect();
        Ok(Value::Array(cleaned?))
    } else {
        Ok(value.clone())
    }
}

/// `RawWithoutParameters` policy: keep raw SQL but strip bind values.
fn sanitize_raw_no_params(value: &Value) -> Result<Value, MetricsError> {
    check_always_sensitive(value)?;
    if let Value::Object(map) = value {
        let mut cleaned = map.clone();
        strip_bind_value_fields(&mut cleaned);
        for v in cleaned.values_mut() {
            *v = sanitize_raw_no_params(v)?;
        }
        Ok(Value::Object(cleaned))
    } else if let Value::Array(arr) = value {
        let cleaned: Result<Vec<Value>, MetricsError> =
            arr.iter().map(sanitize_raw_no_params).collect();
        Ok(Value::Array(cleaned?))
    } else {
        Ok(value.clone())
    }
}

/// Remove bind-value fields from a JSON object (case-insensitive key matching).
fn strip_bind_value_fields(map: &mut serde_json::Map<String, Value>) {
    let keys_to_remove: Vec<String> = map
        .keys()
        .filter(|key| {
            let lower = key.to_lowercase();
            BIND_VALUE_FIELDS.iter().any(|f| lower == *f)
        })
        .cloned()
        .collect();
    for key in keys_to_remove {
        map.remove(&key);
    }
}

/// Recursively check for genuinely sensitive field names (password, token, secret).
fn check_always_sensitive(value: &Value) -> Result<(), MetricsError> {
    if let Value::Object(map) = value {
        for key in map.keys() {
            let lower = key.to_lowercase();
            for sensitive in ALWAYS_SENSITIVE_FIELDS {
                if lower == *sensitive {
                    return Err(MetricsError::SensitiveField { field: key.clone() });
                }
            }
        }
        for v in map.values() {
            check_always_sensitive(v)?;
        }
    } else if let Value::Array(arr) = value {
        for v in arr {
            check_always_sensitive(v)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn fingerprint_only_strips_raw_sql() {
        let input = json!({
            "fingerprint": "SELECT * FROM users WHERE id = ?",
            "parameterized_sql": "SELECT * FROM users WHERE id = ?",
            "raw_sql": "SELECT * FROM users WHERE id = 42",
            "exec_count": 10
        });
        let result = sanitize_payload(&input, SqlTextPolicy::FingerprintOnly).unwrap();
        assert!(result.get("raw_sql").is_none());
        assert!(result.get("fingerprint").is_some());
        assert!(result.get("parameterized_sql").is_some());
        assert_eq!(result.get("exec_count").unwrap(), &json!(10));
    }

    #[test]
    fn sensitive_field_detected() {
        let input = json!({
            "password": "hunter2"
        });
        let err = sanitize_payload(&input, SqlTextPolicy::FingerprintOnly).unwrap_err();
        assert!(matches!(err, MetricsError::SensitiveField { .. }));
    }
}
