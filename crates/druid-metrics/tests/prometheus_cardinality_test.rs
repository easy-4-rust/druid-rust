use std::collections::BTreeMap;

use druid_metrics::prometheus::{
    MetricValue, PrometheusMetric, PrometheusSnapshot, ALLOWED_LABELS,
};

#[test]
fn allowed_labels_contain_only_expected_keys() {
    let expected = &["service", "instance", "datasource", "db_type", "driver"];
    assert_eq!(ALLOWED_LABELS, expected);
}

#[test]
fn prometheus_snapshot_validates_clean_labels() {
    let snap = PrometheusSnapshot {
        metrics: vec![PrometheusMetric {
            name: "druid_datasource_exec_count".to_owned(),
            labels: BTreeMap::from([
                ("service".to_owned(), "my_app".to_owned()),
                ("datasource".to_owned(), "primary_db".to_owned()),
                ("db_type".to_owned(), "postgres".to_owned()),
            ]),
            value: MetricValue::Counter(42),
        }],
    };

    assert!(snap.validate_labels().is_ok());
}

#[test]
fn prometheus_snapshot_rejects_forbidden_sql_text_label() {
    let snap = PrometheusSnapshot {
        metrics: vec![PrometheusMetric {
            name: "druid_sql_stat".to_owned(),
            labels: BTreeMap::from([
                ("service".to_owned(), "my_app".to_owned()),
                ("sql_text".to_owned(), "SELECT * FROM users".to_owned()),
            ]),
            value: MetricValue::Counter(1),
        }],
    };

    let result = snap.validate_labels();
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations[0].contains("sql_text"));
}

#[test]
fn prometheus_snapshot_rejects_forbidden_fingerprint_label() {
    let snap = PrometheusSnapshot {
        metrics: vec![PrometheusMetric {
            name: "druid_sql_stat".to_owned(),
            labels: BTreeMap::from([
                ("service".to_owned(), "my_app".to_owned()),
                (
                    "fingerprint".to_owned(),
                    "SELECT * FROM users WHERE id = ?".to_owned(),
                ),
            ]),
            value: MetricValue::Counter(1),
        }],
    };

    let result = snap.validate_labels();
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations[0].contains("fingerprint"));
}

#[test]
fn prometheus_snapshot_rejects_forbidden_request_id_label() {
    let snap = PrometheusSnapshot {
        metrics: vec![PrometheusMetric {
            name: "druid_request".to_owned(),
            labels: BTreeMap::from([
                ("service".to_owned(), "my_app".to_owned()),
                ("request_id".to_owned(), "abc-123-def".to_owned()),
            ]),
            value: MetricValue::Counter(1),
        }],
    };

    let result = snap.validate_labels();
    assert!(result.is_err());
}

#[test]
fn prometheus_snapshot_rejects_bind_values_in_label() {
    let snap = PrometheusSnapshot {
        metrics: vec![PrometheusMetric {
            name: "druid_sql_stat".to_owned(),
            labels: BTreeMap::from([
                ("service".to_owned(), "my_app".to_owned()),
                ("bind_values".to_owned(), "[42, 'alice']".to_owned()),
            ]),
            value: MetricValue::Counter(1),
        }],
    };

    let result = snap.validate_labels();
    assert!(result.is_err());
}

#[test]
fn prometheus_snapshot_multiple_metrics_validated_together() {
    let snap = PrometheusSnapshot {
        metrics: vec![
            PrometheusMetric {
                name: "druid_exec_count".to_owned(),
                labels: BTreeMap::from([("service".to_owned(), "app".to_owned())]),
                value: MetricValue::Counter(10),
            },
            PrometheusMetric {
                name: "druid_bad_metric".to_owned(),
                labels: BTreeMap::from([("password".to_owned(), "secret".to_owned())]),
                value: MetricValue::Counter(1),
            },
        ],
    };

    let result = snap.validate_labels();
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations[0].contains("password"));
}

#[test]
fn prometheus_empty_snapshot_is_valid() {
    let snap = PrometheusSnapshot::new();
    assert!(snap.validate_labels().is_ok());
}
