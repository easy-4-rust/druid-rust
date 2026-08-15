use druid::stats::{StatFilter, StatsCollector};
use std::sync::Arc;
use std::time::Duration;

fn make_filter() -> StatFilter {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_secs(10)));
    StatFilter::new(collector)
}

// ── new ────────────────────────────────────────────────────────

#[test]
fn stat_filter_new_defaults() {
    let f = make_filter();
    assert!(!f.is_merge_sql());
    assert_eq!(f.get_slow_sql_millis(), 3000);
    assert!(!f.is_log_slow_sql());
    assert_eq!(f.get_slow_sql_log_level(), "ERROR");
    assert!(!f.is_connection_stack_trace_enable());
    assert!(f.get_db_type().is_none());
}

// ── merge_sql getter/setter ────────────────────────────────────

#[test]
fn stat_filter_merge_sql_toggle() {
    let f = make_filter();
    f.set_merge_sql(true);
    assert!(f.is_merge_sql());
    f.set_merge_sql(false);
    assert!(!f.is_merge_sql());
}

// ── slow_sql_millis getter/setter ──────────────────────────────

#[test]
fn stat_filter_slow_sql_millis() {
    let f = make_filter();
    f.set_slow_sql_millis(5000);
    assert_eq!(f.get_slow_sql_millis(), 5000);
}

#[test]
fn stat_filter_slow_sql_millis_negative() {
    let f = make_filter();
    f.set_slow_sql_millis(-1);
    assert_eq!(f.get_slow_sql_millis(), -1);
}

// ── log_slow_sql getter/setter ─────────────────────────────────

#[test]
fn stat_filter_log_slow_sql_toggle() {
    let f = make_filter();
    f.set_log_slow_sql(true);
    assert!(f.is_log_slow_sql());
    f.set_log_slow_sql(false);
    assert!(!f.is_log_slow_sql());
}

// ── slow_sql_log_level getter/setter ───────────────────────────

#[test]
fn stat_filter_slow_sql_log_level_valid() {
    let f = make_filter();
    for level in &["ERROR", "WARN", "INFO", "DEBUG"] {
        f.set_slow_sql_log_level(level);
        assert_eq!(f.get_slow_sql_log_level(), *level);
    }
}

#[test]
fn stat_filter_slow_sql_log_level_case_insensitive() {
    let f = make_filter();
    f.set_slow_sql_log_level("warn");
    assert_eq!(f.get_slow_sql_log_level(), "WARN");
}

#[test]
fn stat_filter_slow_sql_log_level_invalid_ignored() {
    let f = make_filter();
    f.set_slow_sql_log_level("INVALID");
    assert_eq!(f.get_slow_sql_log_level(), "ERROR");
}

// ── connection_stack_trace_enable getter/setter ────────────────

#[test]
fn stat_filter_connection_stack_trace() {
    let f = make_filter();
    f.set_connection_stack_trace_enable(true);
    assert!(f.is_connection_stack_trace_enable());
    f.set_connection_stack_trace_enable(false);
    assert!(!f.is_connection_stack_trace_enable());
}

// ── db_type getter/setter ──────────────────────────────────────

#[test]
fn stat_filter_db_type() {
    let f = make_filter();
    f.set_db_type(Some("mysql"));
    assert_eq!(f.get_db_type(), Some("mysql".to_owned()));
    f.set_db_type(None);
    assert!(f.get_db_type().is_none());
}

// ── merge_sql method ───────────────────────────────────────────

#[test]
fn stat_filter_merge_sql_method_disabled() {
    let f = make_filter();
    f.set_merge_sql(false);
    let result = f.merge_sql("SELECT * FROM t WHERE id = 1", None);
    assert_eq!(result, "SELECT * FROM t WHERE id = 1");
}

#[test]
fn stat_filter_merge_sql_method_enabled() {
    let f = make_filter();
    f.set_merge_sql(true);
    let result = f.merge_sql("SELECT * FROM t WHERE id = 1", None);
    // 参数化后数字被替换为 ?
    assert!(result.contains('?'));
}

// ── result_set_stat ────────────────────────────────────────────

#[test]
fn stat_filter_result_set_stat() {
    let f = make_filter();
    let _ = f.result_set_stat();
}
