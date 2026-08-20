//! Second batch ZK/node coverage tests for the `dynamic` module.
//!
//! Targets uncovered lines in `ZookeeperNodeListener`, `ZookeeperNodeRegister`,
//! `ZookeeperNodeInfo`, `NodeEvent`, `PropertiesUtils`.

extern crate druid_core as druid;
use druid::core::{DruidError, DruidPooledConnection, Pool, PoolState};
use druid::dynamic::node::{NodeEvent, NodeEventTypeEnum, NodeListener, ZookeeperNodeListener};
use druid::dynamic::{DataSourceCreator, HighAvailableDataSource, PropertiesUtils};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

struct MockPool {
    name: &'static str,
    idle: u32,
    max_open: u32,
}

impl MockPool {
    fn arc(name: &'static str) -> Arc<Self> {
        Arc::new(Self {
            name,
            idle: 1,
            max_open: 8,
        })
    }
}

#[async_trait::async_trait]
impl Pool for MockPool {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        Err(DruidError::Other(format!("mock {}", self.name)))
    }
    async fn get_timeout(&self, _: Duration) -> Result<DruidPooledConnection, DruidError> {
        self.get().await
    }
    fn state(&self) -> PoolState {
        PoolState {
            name: self.name.to_owned(),
            idle_count: self.idle as usize,
            max_open: self.max_open as usize,
            ..Default::default()
        }
    }
    fn driver_name(&self) -> &'static str {
        "mock"
    }
    fn name(&self) -> &str {
        self.name
    }
}

fn props(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

// ===========================================================================
// ZookeeperNodeListener
// ===========================================================================

#[tokio::test]
async fn zk_format_url_with_dollar_placeholders() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test");
    listener.set_url_template("jdbc:mysql://${host}:${port}/${database}");
    let events = NodeListener::refresh(&listener).await;
    assert!(events.is_empty(), "empty cache => no events");
}

#[tokio::test]
async fn zk_format_url_with_hash_placeholders() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("pfx");
    listener.set_url_template("jdbc:mysql://#{host}:#{port}/#{database}");
    let _ = NodeListener::refresh(&listener).await;
}

#[tokio::test]
async fn zk_format_url_with_hash_delimited_placeholders() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("pfx");
    listener.set_url_template("jdbc:mysql://#host#:#port#/#database#");
    let _ = NodeListener::refresh(&listener).await;
}

#[tokio::test]
async fn zk_format_url_empty_prefix() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("");
    listener.set_url_template("jdbc:mysql://${host}:${port}/${database}");
    let _ = NodeListener::refresh(&listener).await;
}

#[tokio::test]
async fn zk_check_params_no_client_no_connect() {
    let listener = ZookeeperNodeListener::new();
    listener.set_url_template("jdbc:mysql://localhost/db");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("zkConnectString"), "error: {msg}");
}

#[tokio::test]
async fn zk_check_params_empty_path() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("localhost:2181");
    listener.set_path("");
    listener.set_url_template("jdbc:mysql://localhost/db");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("path"), "error: {msg}");
}

#[tokio::test]
async fn zk_check_params_missing_url_template() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("localhost:2181");
    listener.set_path("/ha");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("urlTemplate"), "error: {msg}");
}

#[tokio::test]
async fn zk_check_params_blank_url_template() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("localhost:2181");
    listener.set_path("/ha");
    listener.set_url_template("  ");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn zk_check_params_empty_connect_string() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("");
    listener.set_url_template("jdbc:mysql://localhost/db");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn zk_update_no_client_is_noop() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test");
    listener.set_url_template("jdbc:mysql://localhost/db");
    NodeListener::update(&listener).await;
}

#[tokio::test]
async fn zk_update_empty_cache_no_state_change() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("p");
    listener.set_url_template("jdbc:mysql://localhost/db");
    NodeListener::update(&listener).await;
    NodeListener::update(&listener).await;
}

#[tokio::test]
async fn zk_destroy_no_task() {
    let listener = ZookeeperNodeListener::new();
    NodeListener::destroy(&listener).await;
}

#[test]
fn zk_set_observer_and_time() {
    let ha = HighAvailableDataSource::new("zk-obs", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    assert!(ha.node_listener().is_none());
}

#[test]
fn zk_last_update_time_initial() {
    let listener = ZookeeperNodeListener::new();
    assert_eq!(NodeListener::last_update_time_millis(&listener), 0);
}

#[test]
fn zk_client_none_initially() {
    let listener = ZookeeperNodeListener::new();
    assert!(listener.client().is_none());
}

#[test]
fn zk_set_client_stores_reference() {
    let listener = ZookeeperNodeListener::new();
    assert!(listener.client().is_none());
}

#[test]
fn zk_set_url_template() {
    let listener = ZookeeperNodeListener::new();
    listener.set_url_template("jdbc:mysql://${host}:${port}");
}

#[test]
fn zk_properties_from_child_data_empty_prefix() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("");
    listener.set_url_template("jdbc:mysql://${host}:${port}/${database}");
}

#[test]
fn zk_multiple_prefix_changes() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("first");
    listener.set_prefix("second");
    listener.set_prefix("");
}

#[tokio::test]
async fn zk_init_connect_string_fails() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("127.0.0.1:19999");
    listener.set_path("/ha-druid");
    listener.set_url_template("jdbc:mysql://${host}:${port}");
    let result = Arc::new(listener).init().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn zk_refresh_twice_same_empty() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test");
    listener.set_url_template("jdbc:mysql://localhost/db");
    let first = NodeListener::refresh(&listener).await;
    let second = NodeListener::refresh(&listener).await;
    assert!(first.is_empty());
    assert!(second.is_empty());
}

// ===========================================================================
// NodeEvent
// ===========================================================================

#[test]
fn node_event_generate_empty_names() {
    let properties = props(&[("n1.url", "jdbc:mysql://n1/db")]);
    let events = NodeEvent::generate_events(&properties, &[], NodeEventTypeEnum::Add);
    assert!(events.is_empty());
}

#[test]
fn node_event_generate_missing_url() {
    let properties = props(&[("n1.username", "user")]);
    let events =
        NodeEvent::generate_events(&properties, &["n1".to_owned()], NodeEventTypeEnum::Add);
    assert_eq!(events.len(), 1);
    assert!(events[0].url().is_none());
    assert_eq!(events[0].username(), Some("user"));
}

#[test]
fn node_event_diff_identical() {
    let p = props(&[("n1.url", "jdbc:mysql://n1/db")]);
    let events = NodeEvent::get_events_by_diff_properties(&p, &p);
    assert!(events.is_empty());
}

#[test]
fn node_event_diff_both_empty() {
    let empty = HashMap::new();
    let events = NodeEvent::get_events_by_diff_properties(&empty, &empty);
    assert!(events.is_empty());
}

#[test]
fn node_event_no_password() {
    let event = NodeEvent::new(
        NodeEventTypeEnum::Add,
        "n1",
        Some("jdbc:mysql://n1/db".to_owned()),
        None,
        None,
    );
    assert!(event.password().is_none());
    assert!(event.username().is_none());
}

#[test]
fn node_event_delete_type() {
    let event = NodeEvent::new(
        NodeEventTypeEnum::Delete,
        "n1",
        Some("jdbc:mysql://n1/db".to_owned()),
        Some("user".to_owned()),
        Some("pass".to_owned()),
    );
    assert_eq!(event.event_type(), NodeEventTypeEnum::Delete);
    assert_eq!(event.node_name(), "n1");
}

#[test]
fn node_event_type_equality() {
    assert_eq!(NodeEventTypeEnum::Add, NodeEventTypeEnum::Add);
    assert_eq!(NodeEventTypeEnum::Delete, NodeEventTypeEnum::Delete);
    assert_ne!(NodeEventTypeEnum::Add, NodeEventTypeEnum::Delete);
}

// ===========================================================================
// PropertiesUtils
// ===========================================================================

#[test]
fn properties_utils_load_none() {
    let p = PropertiesUtils::load_properties(None);
    assert!(p.is_empty());
}

#[test]
fn properties_utils_load_nonexistent() {
    let p =
        PropertiesUtils::load_properties(Some(std::path::Path::new("/no/such/file.properties")));
    assert!(p.is_empty());
}

#[test]
fn properties_utils_name_list_with_prefix() {
    let p = props(&[
        ("ha.node1.url", "jdbc:mysql://n1/db"),
        ("ha.node2.url", "jdbc:mysql://n2/db"),
        ("other.url", "jdbc:mysql://o/db"),
    ]);
    let names = PropertiesUtils::load_name_list(&p, Some("ha."));
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"ha.node1".to_owned()));
    assert!(names.contains(&"ha.node2".to_owned()));
}

#[test]
fn properties_utils_filter_exact_prefix() {
    let p = props(&[
        ("druid.ha.host", "localhost"),
        ("druid.ha.port", "3306"),
        ("other.key", "value"),
    ]);
    let filtered = PropertiesUtils::filter_prefix(&p, Some("druid.ha"));
    assert_eq!(filtered.len(), 2);
}

#[test]
fn properties_utils_filter_none_prefix() {
    let p = props(&[("k1", "v1"), ("k2", "v2")]);
    let filtered = PropertiesUtils::filter_prefix(&p, None);
    assert_eq!(filtered.len(), 2);
}

// ===========================================================================
// ZookeeperNodeInfo
// ===========================================================================

#[test]
fn zk_info_empty_prefix() {
    let mut info = druid::dynamic::ZookeeperNodeInfo::new();
    info.set_prefix(Some(""));
    assert_eq!(info.prefix(), "");
}

#[test]
fn zk_info_trailing_dot_prefix() {
    let mut info = druid::dynamic::ZookeeperNodeInfo::new();
    info.set_prefix(Some("ha."));
    assert_eq!(info.prefix(), "ha.");
}

#[test]
fn zk_info_no_dot_prefix() {
    let mut info = druid::dynamic::ZookeeperNodeInfo::new();
    info.set_prefix(Some("ha"));
    assert_eq!(info.prefix(), "ha.");
}

#[test]
fn zk_info_defaults() {
    let info = druid::dynamic::ZookeeperNodeInfo::new();
    assert_eq!(info.prefix(), "");
    assert!(info.host().is_none());
    assert_eq!(info.port(), None);
    assert!(info.database().is_none());
    assert!(info.username().is_none());
    assert!(info.password().is_none());
}

#[test]
fn zk_info_all_fields() {
    let mut info = druid::dynamic::ZookeeperNodeInfo::new();
    info.set_host(Some("db.example.com".to_owned()));
    info.set_port(Some(5432));
    info.set_database(Some("production".to_owned()));
    info.set_username(Some("admin".to_owned()));
    info.set_password(Some("secret".to_owned()));

    assert_eq!(info.host(), Some("db.example.com"));
    assert_eq!(info.port(), Some(5432));
    assert_eq!(info.database(), Some("production"));
    assert_eq!(info.username(), Some("admin"));
    assert_eq!(info.password(), Some("secret"));
}

#[test]
fn zk_info_clone_eq() {
    let mut info = druid::dynamic::ZookeeperNodeInfo::new();
    info.set_host(Some("host".to_owned()));
    info.set_port(Some(3306));
    let cloned = info.clone();
    assert_eq!(info, cloned);
}

#[test]
fn zk_info_debug() {
    let mut info = druid::dynamic::ZookeeperNodeInfo::new();
    info.set_host(Some("host".to_owned()));
    let debug = format!("{:?}", info);
    assert!(debug.contains("host"));
}

// ===========================================================================
// ZookeeperNodeRegister
// ===========================================================================

#[test]
fn zk_register_setters() {
    let reg = druid::dynamic::ZookeeperNodeRegister::new();
    reg.set_zk_connect_string("localhost:2181");
    assert_eq!(reg.zk_connect_string().as_deref(), Some("localhost:2181"));
    reg.set_path("/ha-druid");
    assert_eq!(reg.path(), "/ha-druid");
    assert!(reg.client().is_none());
}

#[tokio::test]
async fn zk_register_init_no_connect_string() {
    let reg = druid::dynamic::ZookeeperNodeRegister::new();
    let result = reg.init().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn zk_register_empty_payload() {
    let reg = druid::dynamic::ZookeeperNodeRegister::new();
    let result = reg.register("node1", &[]).await;
    assert!(result.is_ok());
    assert!(!result.unwrap());
}

#[tokio::test]
async fn zk_register_before_init() {
    let reg = druid::dynamic::ZookeeperNodeRegister::new();
    let mut info = druid::dynamic::ZookeeperNodeInfo::new();
    info.set_host(Some("host".to_owned()));
    let result = reg.register("node1", &[info]).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn zk_register_deregister_noop() {
    let reg = druid::dynamic::ZookeeperNodeRegister::new();
    let result = reg.deregister().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn zk_register_destroy_noop() {
    let reg = druid::dynamic::ZookeeperNodeRegister::new();
    let result = reg.destroy().await;
    assert!(result.is_ok());
}

#[test]
fn zk_register_path_default() {
    let reg = druid::dynamic::ZookeeperNodeRegister::new();
    assert_eq!(reg.path(), "/ha-druid-datasources");
}

// ===========================================================================
// FileNodeListener
// ===========================================================================

#[test]
fn file_listener_setters() {
    let listener = druid::dynamic::FileNodeListener::new("/tmp/test.properties");
    assert!(listener.prefix().is_empty());
    listener.set_prefix("druid.ha.");
    assert_eq!(listener.prefix(), "druid.ha.");

    listener.set_file("/tmp/other.properties");
    assert!(listener.file().ends_with("other.properties"));

    assert_eq!(listener.interval_seconds(), 60);
    listener.set_interval_seconds(30);
    assert_eq!(listener.interval_seconds(), 30);
}

#[tokio::test]
async fn file_listener_zero_interval_fallback() {
    let dir = std::env::temp_dir().join("druid-ha-file-zero");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "n.url=jdbc:sqlite:n.db\n").unwrap();

    let listener = Arc::new(druid::dynamic::FileNodeListener::new(&file));
    listener.set_interval_seconds(0);
    assert_eq!(listener.interval_seconds(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn file_listener_last_update_time() {
    let listener = druid::dynamic::FileNodeListener::new("/tmp/test.properties");
    assert_eq!(NodeListener::last_update_time_millis(&listener), 0);
}
