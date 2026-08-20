#![cfg(feature = "config-http")]

extern crate druid_core as druid;
use druid_core::core::ConfigFilter;
use std::collections::HashMap;

// ── ConfigFilter::is_enabled ───────────────────────────────────

#[test]
fn config_filter_is_enabled_no_filters() {
    let props = HashMap::new();
    assert!(!ConfigFilter::is_enabled(&props));
}

#[test]
fn config_filter_is_enabled_empty_filters() {
    let mut props = HashMap::new();
    props.insert("filters".to_owned(), String::new());
    assert!(!ConfigFilter::is_enabled(&props));
}

#[test]
fn config_filter_is_enabled_with_config() {
    let mut props = HashMap::new();
    props.insert("filters".to_owned(), "stat,config".to_owned());
    assert!(ConfigFilter::is_enabled(&props));
}

#[test]
fn config_filter_is_enabled_with_class_name() {
    let mut props = HashMap::new();
    props.insert(
        "filters".to_owned(),
        "com.alibaba.druid.filter.config.ConfigFilter".to_owned(),
    );
    assert!(ConfigFilter::is_enabled(&props));
}

#[test]
fn config_filter_is_enabled_with_bang_prefix() {
    let mut props = HashMap::new();
    props.insert("filters".to_owned(), "!config".to_owned());
    assert!(ConfigFilter::is_enabled(&props));
}

#[test]
fn config_filter_is_enabled_without_config() {
    let mut props = HashMap::new();
    props.insert("filters".to_owned(), "stat,wall".to_owned());
    assert!(!ConfigFilter::is_enabled(&props));
}

// ── ConfigFilter::is_decrypt ───────────────────────────────────

#[test]
fn config_filter_is_decrypt_no_props() {
    let filter = ConfigFilter::new();
    let conn = HashMap::new();
    let sys = HashMap::new();
    assert!(!filter.is_decrypt(&conn, None, &sys));
}

#[test]
fn config_filter_is_decrypt_true() {
    let filter = ConfigFilter::new();
    let mut conn = HashMap::new();
    conn.insert("config.decrypt".to_owned(), "true".to_owned());
    let sys = HashMap::new();
    assert!(filter.is_decrypt(&conn, None, &sys));
}

#[test]
fn config_filter_is_decrypt_false() {
    let filter = ConfigFilter::new();
    let mut conn = HashMap::new();
    conn.insert("config.decrypt".to_owned(), "false".to_owned());
    let sys = HashMap::new();
    assert!(!filter.is_decrypt(&conn, None, &sys));
}

#[test]
fn config_filter_is_decrypt_case_insensitive() {
    let filter = ConfigFilter::new();
    let mut conn = HashMap::new();
    conn.insert("config.decrypt".to_owned(), "TRUE".to_owned());
    let sys = HashMap::new();
    assert!(filter.is_decrypt(&conn, None, &sys));
}

#[test]
fn config_filter_is_decrypt_from_config_file() {
    let filter = ConfigFilter::new();
    let conn = HashMap::new();
    let mut config_file = HashMap::new();
    config_file.insert("config.decrypt".to_owned(), "true".to_owned());
    let sys = HashMap::new();
    assert!(filter.is_decrypt(&conn, Some(&config_file), &sys));
}

#[test]
fn config_filter_is_decrypt_from_system() {
    let filter = ConfigFilter::new();
    let conn = HashMap::new();
    let mut sys = HashMap::new();
    sys.insert("druid.config.decrypt".to_owned(), "true".to_owned());
    assert!(filter.is_decrypt(&conn, None, &sys));
}

#[test]
fn config_filter_is_decrypt_conn_takes_precedence() {
    let filter = ConfigFilter::new();
    let mut conn = HashMap::new();
    conn.insert("config.decrypt".to_owned(), "false".to_owned());
    let mut config_file = HashMap::new();
    config_file.insert("config.decrypt".to_owned(), "true".to_owned());
    let sys = HashMap::new();
    assert!(!filter.is_decrypt(&conn, Some(&config_file), &sys));
}

// ── ConfigFilter::new / with_runtime ───────────────────────────

#[test]
fn config_filter_new() {
    let filter = ConfigFilter::new();
    let _ = filter;
}

#[test]
fn config_filter_with_runtime() {
    let client = reqwest::Client::new();
    let filter = ConfigFilter::with_http_client(client, vec![]);
    let _ = filter;
}
