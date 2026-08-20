use std::collections::BTreeMap;

/// Allowed Prometheus label names for Druid metrics.
///
/// SQL text, fingerprints, request IDs, and bind values must NEVER appear
/// as label values to prevent cardinality explosion and data leakage.
pub const ALLOWED_LABELS: &[&str] = &["service", "instance", "datasource", "db_type", "driver"];

/// Prometheus-style metric family.
#[derive(Debug, Clone)]
pub struct PrometheusMetric {
    /// Metric name (e.g. "`druid_datasource_exec_count`").
    pub name: String,
    /// Label set (only keys from [`ALLOWED_LABELS`]).
    pub labels: BTreeMap<String, String>,
    /// Metric value.
    pub value: MetricValue,
}

/// Prometheus metric value types.
#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
}

/// A batch of Prometheus metrics ready for export.
#[derive(Debug, Clone)]
pub struct PrometheusSnapshot {
    pub metrics: Vec<PrometheusMetric>,
}

impl PrometheusSnapshot {
    pub fn new() -> Self {
        Self {
            metrics: Vec::new(),
        }
    }

    /// Validate that no metric contains forbidden label keys.
    pub fn validate_labels(&self) -> Result<(), Vec<String>> {
        let mut violations = Vec::new();
        for metric in &self.metrics {
            for key in metric.labels.keys() {
                if !ALLOWED_LABELS.contains(&key.as_str()) {
                    violations.push(format!(
                        "metric '{}' contains forbidden label '{}'",
                        metric.name, key
                    ));
                }
            }
        }
        if violations.is_empty() {
            Ok(())
        } else {
            Err(violations)
        }
    }
}

impl Default for PrometheusSnapshot {
    fn default() -> Self {
        Self::new()
    }
}
