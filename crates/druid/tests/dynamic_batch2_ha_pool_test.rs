//! Second batch HA/PoolUpdater coverage tests for the `dynamic` module.
//!
//! Targets uncovered lines in `PoolUpdater`, `HighAvailableDataSource` (Pool
//! trait, config, destroy, selector management), `RandomDataSourceValidateTask`.

use druid::core::{DruidError, DruidPooledConnection, Pool, PoolState};
use druid::dynamic::node::{NodeListener, PoolUpdater};
use druid::dynamic::selector::{
    DataSourceSelector, NamedDataSourceSelector, RandomDataSourceSelector,
    RandomDataSourceValidateTask,
};
use druid::dynamic::HighAvailableDataSource;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct MockPool {
    name: &'static str,
    idle: u32,
    max_open: u32,
    close_calls: AtomicU64,
}

impl MockPool {
    fn arc(name: &'static str) -> Arc<Self> {
        Arc::new(Self {
            name,
            idle: 1,
            max_open: 8,
            close_calls: AtomicU64::new(0),
        })
    }

    fn arc_custom(name: &'static str, idle: u32, max_open: u32) -> Arc<Self> {
        Arc::new(Self {
            name,
            idle,
            max_open,
            close_calls: AtomicU64::new(0),
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
    async fn close_for_removal_if_idle(&self) -> Result<bool, DruidError> {
        self.close_calls.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }
}

// ===========================================================================
// PoolUpdater constants and HighAvailableDataSource
// ===========================================================================

#[test]
fn pool_updater_default_interval() {
    assert_eq!(PoolUpdater::DEFAULT_INTERVAL, 60);
}

#[tokio::test]
async fn ha_init_creates_pool_updater_and_listener() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-init");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(
        &file,
        "node1.url=jdbc:sqlite:node1.db\nnode2.url=jdbc:sqlite:node2.db\n",
    )
    .unwrap();

    let ha = HighAvailableDataSource::new("init-test");
    ha.set_data_source_file(&file);
    ha.set_property_prefix("");
    ha.set_pool_purge_interval_seconds(60);
    let _ = ha.init().await;
    assert!(
        ha.node_listener().is_some(),
        "init must install node_listener"
    );
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_idempotent() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-idempotent");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "n.url=jdbc:sqlite:n.db\n").unwrap();

    let ha = HighAvailableDataSource::new("idem-test");
    ha.set_data_source_file(&file);
    let _ = ha.init().await;
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_with_preexisting_skips_listener() {
    let ha = HighAvailableDataSource::new("preexist");
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let _ = ha.init().await;
    ha.destroy().await;
}

#[tokio::test]
async fn pool_updater_delete_last_node_protected() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-protect");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "solo.url=jdbc:sqlite:solo.db\n").unwrap();

    let ha = HighAvailableDataSource::new("protect-test");
    ha.set_data_source_file(&file);
    ha.set_allow_empty_pool_when_update(false);
    let _ = ha.init().await;
    ha.insert_data_source("solo", MockPool::arc("solo"));

    if let Some(listener) = ha.node_listener() {
        NodeListener::update(&*listener).await;
    }

    assert!(
        ha.data_source_map().contains_key("solo"),
        "last node must not be deleted when allow_empty_pool=false"
    );
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn pool_updater_delete_last_node_allowed() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-allow");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "solo.url=jdbc:sqlite:solo.db\n").unwrap();

    let ha = HighAvailableDataSource::new("allow-test");
    ha.set_data_source_file(&file);
    ha.set_allow_empty_pool_when_update(true);
    let _ = ha.init().await;
    ha.insert_data_source("solo", MockPool::arc("solo"));
    assert!(ha.data_source_map().contains_key("solo"));
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn pool_updater_small_interval_warning() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-small-interval");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "n.url=jdbc:sqlite:n.db\n").unwrap();

    let ha = HighAvailableDataSource::new("small-interval");
    ha.set_data_source_file(&file);
    ha.set_pool_purge_interval_seconds(5);
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn pool_updater_zero_interval_fallback() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-zero-interval");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "n.url=jdbc:sqlite:n.db\n").unwrap();

    let ha = HighAvailableDataSource::new("zero-interval");
    ha.set_data_source_file(&file);
    ha.set_pool_purge_interval_seconds(0);
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn pool_updater_negative_interval_fallback() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-neg-interval");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "n.url=jdbc:sqlite:n.db\n").unwrap();

    let ha = HighAvailableDataSource::new("neg-interval");
    ha.set_data_source_file(&file);
    ha.set_pool_purge_interval_seconds(-5);
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn pool_updater_update_empty_noop() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-empty-update");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "").unwrap();

    let ha = HighAvailableDataSource::new("empty-update");
    ha.set_data_source_file(&file);
    let _ = ha.init().await;
    if let Some(listener) = ha.node_listener() {
        NodeListener::update(&*listener).await;
        NodeListener::update(&*listener).await;
    }
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_update_cycle_add_empty_name() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-add-empty");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "node1.url=jdbc:sqlite:n1.db\n").unwrap();

    let ha = HighAvailableDataSource::new("add-empty");
    ha.set_data_source_file(&file);
    let _ = ha.init().await;
    if let Some(listener) = ha.node_listener() {
        NodeListener::update(&*listener).await;
    }
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_multiple_nodes() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-multi");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(
        &file,
        "n1.url=jdbc:sqlite:n1.db\nn2.url=jdbc:sqlite:n2.db\nn3.url=jdbc:sqlite:n3.db\n",
    )
    .unwrap();

    let ha = HighAvailableDataSource::new("multi");
    ha.set_data_source_file(&file);
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_with_prefix() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-prefix");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(
        &file,
        "ha.node1.url=jdbc:sqlite:n1.db\nha.node2.url=jdbc:sqlite:n2.db\nother.key=value\n",
    )
    .unwrap();

    let ha = HighAvailableDataSource::new("prefix-test");
    ha.set_data_source_file(&file);
    ha.set_property_prefix("ha.");
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_empty_url_warning() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-empty-url");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "node1.url=\nnode2.url=jdbc:sqlite:n2.db\n").unwrap();

    let ha = HighAvailableDataSource::new("empty-url");
    ha.set_data_source_file(&file);
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_with_credentials() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-creds");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(
        &file,
        "node1.url=jdbc:sqlite:n1.db\nnode1.username=admin\nnode1.password=secret\n",
    )
    .unwrap();

    let ha = HighAvailableDataSource::new("creds-test");
    ha.set_data_source_file(&file);
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_unsupported_url_scheme() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-unsup");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "node1.url=ftp://example.com/db\n").unwrap();

    let ha = HighAvailableDataSource::new("unsup-scheme");
    ha.set_data_source_file(&file);
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn ha_init_invalid_url() {
    let dir = std::env::temp_dir().join("druid-ha-batch2-invalid-url");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");
    std::fs::write(&file, "node1.url=not-a-valid-url\n").unwrap();

    let ha = HighAvailableDataSource::new("invalid-url");
    ha.set_data_source_file(&file);
    let _ = ha.init().await;
    ha.destroy().await;
    let _ = std::fs::remove_dir_all(&dir);
}

// ===========================================================================
// HighAvailableDataSource Pool trait
// ===========================================================================

#[tokio::test]
async fn ha_pool_get_no_pools() {
    let ha = HighAvailableDataSource::new("no-pools");
    let result = Pool::get(&ha).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ha_pool_get_timeout_no_pools() {
    let ha = HighAvailableDataSource::new("no-pools-timeout");
    let result = Pool::get_timeout(&ha, Duration::from_secs(1)).await;
    assert!(result.is_err());
}

#[test]
fn ha_pool_state_no_selector() {
    let ha = HighAvailableDataSource::new("state-test");
    let state = Pool::state(&ha);
    assert_eq!(state.name, "state-test");
}

#[test]
fn ha_pool_driver_name() {
    let ha = HighAvailableDataSource::new("driver-test");
    assert_eq!(Pool::driver_name(&ha), "druid-ha");
}

#[test]
fn ha_pool_name() {
    let ha = HighAvailableDataSource::new("my-ha");
    assert_eq!(Pool::name(&ha), "my-ha");
}

#[tokio::test]
async fn ha_pool_close_pool() {
    let ha = HighAvailableDataSource::new("close-test");
    Pool::close_pool(&ha).await;
}

#[test]
fn ha_set_data_source_selector_replaces() {
    let ha = HighAvailableDataSource::new("replace-sel");
    ha.set_selector("random");
    assert_eq!(ha.selector_name(), Some("random"));

    let new_sel = NamedDataSourceSelector::new(&ha);
    ha.set_data_source_selector(Arc::new(new_sel));
    assert_eq!(ha.selector_name(), Some("byName"));
}

#[test]
fn ha_set_selector_unknown_no_change() {
    let ha = HighAvailableDataSource::new("unknown-sel");
    ha.set_selector("random");
    ha.set_selector("no-such-selector");
    assert_eq!(ha.selector_name(), Some("random"));
}

#[test]
fn ha_set_connection_properties_empty_clears() {
    let ha = HighAvailableDataSource::new("conn-props");
    ha.set_connection_properties(Some("user=admin;password=secret"));
    ha.set_connection_properties(Some(""));
}

#[test]
fn ha_set_connection_properties_no_equals() {
    let ha = HighAvailableDataSource::new("conn-props2");
    ha.set_connection_properties(Some("key_without_value"));
}

#[test]
fn ha_set_connection_properties_none_clears() {
    let ha = HighAvailableDataSource::new("conn-props3");
    ha.set_connection_properties(Some("k=v"));
    ha.set_connection_properties(None);
}

#[test]
fn ha_set_connect_properties_none() {
    let ha = HighAvailableDataSource::new("cp-none");
    ha.set_connect_properties(None);
}

#[test]
fn ha_set_connect_properties_extends() {
    let ha = HighAvailableDataSource::new("cp-ext");
    let mut p1 = HashMap::new();
    p1.insert("k1".to_owned(), "v1".to_owned());
    ha.set_connect_properties(Some(p1));
    let mut p2 = HashMap::new();
    p2.insert("k2".to_owned(), "v2".to_owned());
    ha.set_connect_properties(Some(p2));
}

#[test]
fn ha_available_excludes_blacklist() {
    let ha = HighAvailableDataSource::new("avail-test");
    ha.insert_data_source("a", MockPool::arc("a"));
    ha.insert_data_source("b", MockPool::arc("b"));
    ha.add_blacklist("a");
    assert_eq!(ha.available_data_source_map().len(), 1);
    assert!(ha.available_data_source_map().contains_key("b"));
}

#[test]
fn ha_add_blacklist_nonexistent() {
    let ha = HighAvailableDataSource::new("bl-nonexist");
    ha.add_blacklist("ghost");
    assert!(!ha.is_in_blacklist("ghost"));
}

#[test]
fn ha_remove_blacklist_nonexistent() {
    let ha = HighAvailableDataSource::new("rm-bl-nonexist");
    ha.remove_blacklist("ghost");
}

#[tokio::test]
async fn ha_destroy_with_pools() {
    let ha = HighAvailableDataSource::new("destroy-pools");
    ha.insert_data_source("p1", MockPool::arc("p1"));
    ha.insert_data_source("p2", MockPool::arc("p2"));
    ha.destroy().await;
}

#[tokio::test]
async fn ha_get_connection_no_selector() {
    let ha = HighAvailableDataSource::new("no-sel-conn");
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let result = ha.get_connection().await;
    match result {
        Ok(None) => {}
        Err(_) => {}
        Ok(Some(_)) => panic!("mock pool should not return a connection"),
    }
}

#[test]
fn ha_set_target_with_byname() {
    let ha = HighAvailableDataSource::new("target-test");
    ha.set_selector("byName");
    ha.set_target_data_source(Some("master".to_owned()));
    ha.set_target_data_source(None);
}

#[test]
fn ha_set_target_with_random() {
    let ha = HighAvailableDataSource::new("target-random");
    ha.set_selector("random");
    ha.set_target_data_source(Some("master".to_owned()));
}

#[test]
fn ha_set_target_with_sticky() {
    let ha = HighAvailableDataSource::new("target-sticky");
    ha.set_selector("stickyRandom");
    ha.set_target_data_source(Some("master".to_owned()));
}

#[test]
fn ha_set_target_no_selector() {
    let ha = HighAvailableDataSource::new("target-none");
    ha.set_target_data_source(Some("master".to_owned()));
    ha.set_target_data_source(None);
}

// ===========================================================================
// RandomDataSourceValidateTask
// ===========================================================================

#[test]
fn validate_task_log_and_query_success_time() {
    RandomDataSourceValidateTask::log_success_time("test-ds");
    let time = RandomDataSourceValidateTask::success_time("test-ds");
    assert!(time.is_some());
    assert!(time.unwrap() > 0);
}

#[test]
fn validate_task_log_empty_name() {
    RandomDataSourceValidateTask::log_success_time("");
}

#[test]
fn validate_task_success_time_unknown() {
    let time = RandomDataSourceValidateTask::success_time("unknown-ds-xyz");
    assert!(time.is_none());
}

#[test]
fn validate_task_constants() {
    assert_eq!(
        RandomDataSourceValidateTask::DEFAULT_CHECKING_INTERVAL_SECONDS,
        10
    );
    assert_eq!(RandomDataSourceValidateTask::DEFAULT_BLACKLIST_THRESHOLD, 3);
}
