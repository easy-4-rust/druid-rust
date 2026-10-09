use super::capture_probe_connection::CaptureProbeConnection;
use super::stateful_connection::StatefulConnection;
use super::ConnectionDefaults;
use druid_rdbc::connection::{Connection, TransactionIsolation};
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::ResultSetHoldability;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn probe() -> CaptureProbeConnection {
    CaptureProbeConnection {
        state: StatefulConnection {
            auto_commit: true,
            read_only: false,
            isolation: TransactionIsolation::READ_COMMITTED,
            holdability: ResultSetHoldability::DriverValue(0),
            catalog: "catalog_a".to_owned(),
            schema: "schema_a".to_owned(),
        },
        errors: HashMap::new(),
        calls: Arc::new(Mutex::new(Vec::new())),
    }
}

fn setter_calls(connection: &CaptureProbeConnection) -> Vec<&'static str> {
    connection
        .calls
        .lock()
        .unwrap()
        .iter()
        .copied()
        .filter(|operation| operation.starts_with("set_"))
        .collect()
}

/// 原 BASE 四项状态复位的相对顺序保持，并保留 catalog/schema 的现有复位策略。
#[tokio::test]
async fn reset_keeps_original_state_order_and_preservation_policy() {
    let mut connection = probe();
    let defaults = ConnectionDefaults::capture_standard(&mut connection)
        .await
        .unwrap();
    connection.state.read_only = true;
    connection.state.holdability = ResultSetHoldability::HoldOverCommit;
    connection.state.isolation = TransactionIsolation::SERIALIZABLE;
    connection.state.auto_commit = false;
    connection.state.catalog = "catalog_b".to_owned();
    connection.state.schema = "schema_b".to_owned();
    connection.calls.lock().unwrap().clear();

    defaults
        .reset_standard_state(&mut connection, true, false)
        .await
        .unwrap();
    let calls = setter_calls(&connection);
    let original_state_calls: Vec<_> = calls
        .iter()
        .copied()
        .filter(|operation| !matches!(*operation, "set_catalog" | "set_schema"))
        .collect();
    assert_eq!(
        original_state_calls,
        [
            "set_read_only",
            "set_holdability",
            "set_transaction_isolation",
            "set_auto_commit"
        ]
    );
    assert_eq!(connection.state.catalog, "catalog_a");
    assert_eq!(connection.state.schema, "schema_a");
    assert!(!defaults.standard_needs_reset(&connection, false).unwrap());

    connection.state.isolation = TransactionIsolation::SERIALIZABLE;
    connection.state.schema = "schema_b".to_owned();
    connection.calls.lock().unwrap().clear();
    defaults
        .reset_standard_state(&mut connection, false, true)
        .await
        .unwrap();
    assert!(setter_calls(&connection).is_empty());
    assert_eq!(connection.state.isolation.jdbc_code(), 8);
    assert_eq!(connection.state.schema, "schema_b");
}

/// 不支持的 getter 不能使状态被伪装成可复位；两种标准拒绝分别覆盖。
#[tokio::test]
async fn unsupported_state_is_not_restored_or_reported_dirty() {
    for feature_error in [false, true] {
        let mut connection = probe();
        for operation in ["holdability", "transaction_isolation"] {
            let error = if feature_error {
                RdbcError::feature_not_supported(operation)
            } else {
                RdbcError::UnsupportedOperation { operation }
            };
            connection.errors.insert(operation, error);
        }
        let defaults = ConnectionDefaults::capture_standard(&mut connection)
            .await
            .unwrap();
        connection.state.holdability = ResultSetHoldability::HoldOverCommit;
        connection.state.isolation = TransactionIsolation::SERIALIZABLE;
        connection.calls.lock().unwrap().clear();
        assert!(!defaults.standard_needs_reset(&connection, false).unwrap());
        defaults
            .reset_standard_state(&mut connection, true, false)
            .await
            .unwrap();
        assert!(setter_calls(&connection).is_empty());
        assert_eq!(connection.state.holdability.rdbc_code(), 1);
        assert_eq!(connection.state.isolation.jdbc_code(), 8);
        assert!(!connection
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|operation| matches!(*operation, "holdability" | "transaction_isolation")));
    }
}

/// 真实 driver error 或失效连接不会被 optional-state 逻辑吞成不支持。
#[tokio::test]
async fn actual_state_errors_propagate_from_capture_and_compare() {
    for operation in ["holdability", "transaction_isolation"] {
        for expected in [
            RdbcError::DriverError("state read failed".to_owned()),
            RdbcError::ConnectionDiscarded,
        ] {
            let mut connection = probe();
            connection.errors.insert(operation, expected.clone());
            let error = ConnectionDefaults::capture_standard(&mut connection)
                .await
                .unwrap_err();
            assert_eq!(RdbcError::from(error), expected);

            connection.errors.clear();
            let defaults = ConnectionDefaults::capture_standard(&mut connection)
                .await
                .unwrap();
            connection.errors.insert(operation, expected.clone());
            assert_eq!(
                RdbcError::from(
                    defaults
                        .standard_needs_reset(&connection, false)
                        .unwrap_err()
                ),
                expected
            );
            connection.calls.lock().unwrap().clear();
            assert_eq!(
                RdbcError::from(
                    defaults
                        .reset_standard_state(&mut connection, true, false)
                        .await
                        .unwrap_err()
                ),
                expected
            );
            assert!(setter_calls(&connection).is_empty());
        }
    }
}

/// 复位 setter 失败后立即停止，后续状态仍保持调用前真实值。
#[tokio::test]
async fn reset_stops_at_driver_setter_failure() {
    for failed_operation in ["set_holdability", "set_transaction_isolation"] {
        let mut connection = probe();
        let defaults = ConnectionDefaults::capture_standard(&mut connection)
            .await
            .unwrap();
        connection.state.read_only = true;
        connection.state.holdability = ResultSetHoldability::HoldOverCommit;
        connection.state.isolation = TransactionIsolation::SERIALIZABLE;
        connection.state.auto_commit = false;
        connection.errors.insert(
            failed_operation,
            RdbcError::DriverError("state write failed".to_owned()),
        );
        connection.calls.lock().unwrap().clear();
        let error = defaults
            .reset_standard_state(&mut connection, true, false)
            .await
            .unwrap_err();
        assert_eq!(
            RdbcError::from(error),
            RdbcError::DriverError("state write failed".to_owned())
        );
        assert!(!connection.state.read_only);
        assert!(!connection.state.auto_commit);
        assert_eq!(connection.state.isolation.jdbc_code(), 8);
        let expected_calls = if failed_operation == "set_holdability" {
            vec!["set_read_only", "set_holdability"]
        } else {
            vec![
                "set_read_only",
                "set_holdability",
                "set_transaction_isolation",
            ]
        };
        assert_eq!(setter_calls(&connection), expected_calls);
        assert_eq!(
            connection.state.holdability.rdbc_code(),
            i32::from(failed_operation == "set_holdability")
        );
    }
}

/// 标准 signed 隔离级别原值不被窄化，捕获与复位都委托真实字段。
#[tokio::test]
async fn capture_preserves_signed_isolation_codes_without_truncation() {
    for code in [-1, 0, 255, 256, i32::MAX, i32::MIN] {
        let mut connection = probe();
        connection.state.isolation = TransactionIsolation::from_jdbc(code);
        let defaults = ConnectionDefaults::capture_standard(&mut connection)
            .await
            .unwrap();
        assert_eq!(defaults.transaction_isolation(), code);
        connection.state.isolation = TransactionIsolation::SERIALIZABLE;
        defaults
            .reset_standard_state(&mut connection, true, false)
            .await
            .unwrap();
        assert_eq!(
            connection.transaction_isolation().unwrap().jdbc_code(),
            code
        );
    }
}
