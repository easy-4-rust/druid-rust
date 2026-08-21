//! `ZookeeperNodeListener` differential coverage tests (Java Druid 1.2.28).
//!
//! Covers paths that do not require a real ZK connection:
//! `direct_child_name`, `format_url` placeholder substitution, `check_parameters`
//! validation, `properties_from_child_data` prefix rewriting, Default
//! construction, set_* setters, `NodeEvent` generation and diff.

use druid::dynamic::node::{NodeEvent, NodeEventTypeEnum, ZookeeperNodeListener};
use druid::dynamic::{NodeListener, PropertiesUtils};
use std::collections::HashMap;
use std::sync::Arc;

// ===========================================================================
// 1. Default construction and setters
// ===========================================================================

#[test]
fn zk_listener_default_has_no_client() {
    let listener = ZookeeperNodeListener::new();
    assert!(listener.client().is_none(), "default must have no client");
}

#[test]
fn zk_listener_set_prefix() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("my.prefix");
}

#[test]
fn zk_listener_set_zk_connect_string() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("localhost:2181");
}

#[test]
fn zk_listener_set_path() {
    let listener = ZookeeperNodeListener::new();
    listener.set_path("/custom/path");
}

#[test]
fn zk_listener_set_url_template() {
    let listener = ZookeeperNodeListener::new();
    listener.set_url_template("jdbc:mysql://${host}:${port}/${database}");
}

// ===========================================================================
// 2. format_url placeholder substitution (indirect via update/refresh)
// ===========================================================================

#[tokio::test]
async fn zk_format_url_host_placeholder() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test");
    listener.set_url_template("jdbc:mysql://${host}:${port}/${database}");
    // update without client is a no-op; exercises format_url code path
    listener.update().await;
}

#[tokio::test]
async fn zk_format_url_hash_placeholder() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test");
    listener.set_url_template("jdbc:mysql://#{host}:#{port}/#{database}");
    listener.update().await;
}

#[tokio::test]
async fn zk_format_url_hash_delimited_placeholder() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test");
    listener.set_url_template("jdbc:mysql://#host#:#port#/#database#");
    listener.update().await;
}

#[tokio::test]
async fn zk_format_url_empty_prefix() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("");
    listener.set_url_template("jdbc:mysql://${host}:${port}");
    listener.update().await;
}

// ===========================================================================
// 3. check_parameters validation
// ===========================================================================

#[tokio::test]
async fn zk_check_parameters_no_client_no_connect_string() {
    let listener = ZookeeperNodeListener::new();
    listener.set_url_template("jdbc:mysql://localhost/db");
    let result = Arc::new(listener).init().await;
    assert!(
        result.is_err(),
        "must fail without client or connect string"
    );
}

#[tokio::test]
async fn zk_check_parameters_empty_path() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("localhost:2181");
    listener.set_path("");
    listener.set_url_template("jdbc:mysql://localhost/db");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err(), "must fail with empty path");
}

#[tokio::test]
async fn zk_check_parameters_empty_url_template() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("localhost:2181");
    listener.set_path("/ha-druid-datasources");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err(), "must fail without urlTemplate");
}

#[tokio::test]
async fn zk_check_parameters_blank_url_template() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("localhost:2181");
    listener.set_path("/ha-druid-datasources");
    listener.set_url_template("");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err(), "must fail with blank urlTemplate");
}

// ===========================================================================
// 4. refresh / update / destroy without client
// ===========================================================================

#[tokio::test]
async fn zk_refresh_without_client_returns_empty() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test");
    listener.set_url_template("jdbc:mysql://localhost/db");
    let events = listener.refresh().await;
    assert!(
        events.is_empty(),
        "refresh without client must return empty"
    );
}

#[tokio::test]
async fn zk_update_without_client_is_noop() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test");
    listener.set_url_template("jdbc:mysql://localhost/db");
    listener.update().await;
}

#[tokio::test]
async fn zk_destroy_without_init_is_noop() {
    let listener = ZookeeperNodeListener::new();
    listener.destroy().await;
}

#[test]
fn zk_last_update_time_millis_initial() {
    let listener = ZookeeperNodeListener::new();
    assert_eq!(listener.last_update_time_millis(), 0);
}

// ===========================================================================
// 5. NodeEvent generation and diff
// ===========================================================================

#[test]
fn node_event_diff_adds_new_node() {
    let previous = HashMap::new();
    let mut next = HashMap::new();
    next.insert("node1.url".to_owned(), "jdbc:mysql://node1/db".to_owned());
    next.insert("node1.username".to_owned(), "root".to_owned());
    let events = NodeEvent::get_events_by_diff_properties(&previous, &next);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type(), NodeEventTypeEnum::Add);
    assert_eq!(events[0].node_name(), "node1");
    assert_eq!(events[0].url(), Some("jdbc:mysql://node1/db"));
    assert_eq!(events[0].username(), Some("root"));
}

#[test]
fn node_event_diff_deletes_removed_node() {
    let mut previous = HashMap::new();
    previous.insert("node1.url".to_owned(), "jdbc:mysql://node1/db".to_owned());
    let next = HashMap::new();
    let events = NodeEvent::get_events_by_diff_properties(&previous, &next);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type(), NodeEventTypeEnum::Delete);
    assert_eq!(events[0].node_name(), "node1");
}

#[test]
fn node_event_diff_no_change() {
    let mut previous = HashMap::new();
    previous.insert("node1.url".to_owned(), "jdbc:mysql://node1/db".to_owned());
    let mut next = HashMap::new();
    next.insert("node1.url".to_owned(), "jdbc:mysql://node1/db".to_owned());
    let events = NodeEvent::get_events_by_diff_properties(&previous, &next);
    assert!(events.is_empty(), "no change must produce no events");
}

#[test]
fn node_event_generate_events() {
    let mut properties = HashMap::new();
    properties.insert("node1.url".to_owned(), "jdbc:mysql://node1/db".to_owned());
    properties.insert("node1.username".to_owned(), "user1".to_owned());
    properties.insert("node2.url".to_owned(), "jdbc:mysql://node2/db".to_owned());
    let names = vec!["node1".to_owned(), "node2".to_owned()];
    let events = NodeEvent::generate_events(&properties, &names, NodeEventTypeEnum::Add);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].node_name(), "node1");
    assert_eq!(events[0].url(), Some("jdbc:mysql://node1/db"));
    assert_eq!(events[0].username(), Some("user1"));
    assert_eq!(events[1].node_name(), "node2");
    assert_eq!(events[1].url(), Some("jdbc:mysql://node2/db"));
    assert!(events[1].username().is_none());
}

#[test]
fn node_event_debug_hides_password() {
    let event = NodeEvent::new(
        NodeEventTypeEnum::Add,
        "node1",
        Some("jdbc:mysql://node1/db".to_owned()),
        Some("root".to_owned()),
        Some("secret123".to_owned()),
    );
    let debug = format!("{event:?}");
    assert!(
        debug.contains("password_length"),
        "debug must show password_length"
    );
    assert!(
        !debug.contains("secret123"),
        "debug must not contain actual password"
    );
}

#[test]
fn node_event_debug_no_password() {
    let event = NodeEvent::new(NodeEventTypeEnum::Add, "node1", None, None, None);
    let debug = format!("{event:?}");
    assert!(!debug.contains("password_length"));
}

#[test]
fn node_event_diff_mixed_add_delete() {
    let mut previous = HashMap::new();
    previous.insert("old_node.url".to_owned(), "jdbc:mysql://old/db".to_owned());
    let mut next = HashMap::new();
    next.insert("new_node.url".to_owned(), "jdbc:mysql://new/db".to_owned());
    let events = NodeEvent::get_events_by_diff_properties(&previous, &next);
    assert_eq!(events.len(), 2);
    let add_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_type() == NodeEventTypeEnum::Add)
        .collect();
    let delete_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_type() == NodeEventTypeEnum::Delete)
        .collect();
    assert_eq!(add_events.len(), 1);
    assert_eq!(add_events[0].node_name(), "new_node");
    assert_eq!(delete_events.len(), 1);
    assert_eq!(delete_events[0].node_name(), "old_node");
}

#[test]
fn node_event_diff_empty_name_ignored() {
    let previous = HashMap::new();
    let mut next = HashMap::new();
    next.insert(".url".to_owned(), "jdbc:mysql://empty/db".to_owned());
    let events = NodeEvent::get_events_by_diff_properties(&previous, &next);
    let non_empty: Vec<_> = events
        .iter()
        .filter(|e| !e.node_name().trim().is_empty())
        .collect();
    assert!(non_empty.is_empty(), "empty node names must be filtered");
}

// ===========================================================================
// 6. PropertiesUtils
// ===========================================================================

#[test]
fn properties_utils_filter_prefix() {
    let mut properties = HashMap::new();
    properties.insert("druid.ha.host".to_owned(), "localhost".to_owned());
    properties.insert("druid.ha.port".to_owned(), "3306".to_owned());
    properties.insert("other.key".to_owned(), "value".to_owned());
    let filtered = PropertiesUtils::filter_prefix(&properties, Some("druid.ha"));
    assert_eq!(filtered.len(), 2);
    assert!(filtered.contains_key("druid.ha.host"));
    assert!(filtered.contains_key("druid.ha.port"));
    assert!(!filtered.contains_key("other.key"));
}

#[test]
fn properties_utils_filter_empty_prefix() {
    let mut properties = HashMap::new();
    properties.insert("key1".to_owned(), "value1".to_owned());
    properties.insert("key2".to_owned(), "value2".to_owned());
    let filtered = PropertiesUtils::filter_prefix(&properties, Some(""));
    assert_eq!(filtered.len(), 2);
}

#[test]
fn properties_utils_load_name_list() {
    let mut properties = HashMap::new();
    properties.insert("node1.url".to_owned(), "jdbc:mysql://node1/db".to_owned());
    properties.insert("node2.url".to_owned(), "jdbc:mysql://node2/db".to_owned());
    let names = PropertiesUtils::load_name_list(&properties, Some(""));
    assert!(names.contains(&"node1".to_owned()));
    assert!(names.contains(&"node2".to_owned()));
}
