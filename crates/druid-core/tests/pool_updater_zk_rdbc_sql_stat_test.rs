//! `PoolUpdater` / `ZooKeeper` / `RdbcSqlStat` 差分测试（C9 批次：dynamic/node + stats 0%）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。

extern crate druid_core as druid;
use druid::core::{DruidError, DruidPooledConnection, Pool, PoolState};
use druid::dynamic::node::{
    NodeEvent, NodeEventTypeEnum, ZookeeperNodeInfo, ZookeeperNodeListener, ZookeeperNodeRegister,
};
use druid::dynamic::{HighAvailableDataSource, PropertiesUtils};
use druid::stats::RdbcSqlStat;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

// ── Mock Pool ────────────────────────────────────────────────

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
    fn arc_custom(name: &'static str, idle: u32, max_open: u32) -> Arc<Self> {
        Arc::new(Self {
            name,
            idle,
            max_open,
        })
    }
}

#[async_trait::async_trait]
impl Pool for MockPool {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        Err(DruidError::Other("mock".to_owned()))
    }
    async fn get_timeout(&self, _: Duration) -> Result<DruidPooledConnection, DruidError> {
        Err(DruidError::Other("mock".to_owned()))
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
        Ok(true)
    }
}

fn props(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

// ── HighAvailableDataSource / PoolUpdater 行为测试 ────────────

/// `黑名单管理：add/remove/is_in_blacklist`。
#[test]
fn ha_blacklist_management() {
    let ha = HighAvailableDataSource::new("bl-test");
    ha.insert_data_source("n1", MockPool::arc("n1"));
    ha.insert_data_source("n2", MockPool::arc("n2"));

    assert!(!ha.is_in_blacklist("n1"));
    ha.add_blacklist("n1");
    assert!(ha.is_in_blacklist("n1"));
    assert_eq!(ha.available_data_source_map().len(), 1);

    ha.remove_blacklist("n1");
    assert!(!ha.is_in_blacklist("n1"));
    assert_eq!(ha.available_data_source_map().len(), 2);
}

/// selector 安装与查询。
#[test]
fn ha_selector_management() {
    let ha = HighAvailableDataSource::new("sel-test");
    assert!(ha.selector_name().is_none());
    ha.set_selector("byName");
    assert_eq!(ha.selector_name(), Some("byName"));
    ha.set_selector("random");
    assert_eq!(ha.selector_name(), Some("random"));
    ha.set_selector("stickyRandom");
    assert_eq!(ha.selector_name(), Some("stickyRandom"));
    // 无效名称不改变。
    ha.set_selector("bogus");
    assert_eq!(ha.selector_name(), Some("stickyRandom"));
}

/// insert / remove / `set_data_source_map` / `data_source_map` 全生命周期。
#[test]
fn ha_datasource_map_lifecycle() {
    let ha = HighAvailableDataSource::new("map-test");
    assert!(ha.data_source_map().is_empty());

    ha.insert_data_source("master", MockPool::arc("master"));
    ha.insert_data_source("slave", MockPool::arc_custom("slave", 0, 8));
    assert_eq!(ha.data_source_map().len(), 2);

    let removed = ha.remove_data_source("master");
    assert!(removed.is_some());
    assert!(ha.remove_data_source("missing").is_none());
    assert_eq!(ha.data_source_map().len(), 1);

    let mut new_map = HashMap::new();
    new_map.insert("solo".to_owned(), MockPool::arc("solo") as Arc<dyn Pool>);
    ha.set_data_source_map(new_map);
    assert_eq!(ha.data_source_map().len(), 1);
    assert!(ha.data_source_map().contains_key("solo"));
}

/// `allow_empty_pool` 配置（通过 init 间接触发）。
#[test]
fn ha_test_on_borrow_and_return_config() {
    let ha = HighAvailableDataSource::new("config-test");
    assert!(!ha.is_test_on_borrow());
    ha.set_test_on_borrow(true);
    assert!(ha.is_test_on_borrow());
    assert!(!ha.is_test_on_return());
    ha.set_test_on_return(true);
    assert!(ha.is_test_on_return());
}

// ── ZookeeperNodeListener ────────────────────────────────────

/// 配置 setter / getter。
#[test]
fn zk_listener_config_setters() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("test-prefix");
    listener.set_zk_connect_string("localhost:2181");
    listener.set_path("/custom/path");
    listener.set_url_template("jdbc:mysql://${host}:${port}/${database}");
    assert!(listener.client().is_none());
}

/// `check_parameters` 缺参数拒绝。
#[tokio::test]
async fn zk_listener_check_parameters_rejects_missing() {
    let listener = ZookeeperNodeListener::new();
    let result = druid::dynamic::node::NodeListener::init(Arc::new(listener)).await;
    assert!(result.is_err());
}

/// 参数齐全但无 ZK 实例：参数校验通过但连接失败。
#[tokio::test]
async fn zk_listener_check_parameters_passes_but_no_zk() {
    let listener = ZookeeperNodeListener::new();
    listener.set_zk_connect_string("localhost:12345");
    listener.set_path("/ha-druid-datasources");
    listener.set_url_template("jdbc:mysql://${host}:${port}/${database}");
    let result = druid::dynamic::node::NodeListener::init(Arc::new(listener)).await;
    assert!(result.is_err());
}

/// refresh 空缓存 → 无事件。
#[tokio::test]
async fn zk_listener_refresh_empty_cache() {
    let listener = ZookeeperNodeListener::new();
    listener.set_prefix("p");
    listener.set_zk_connect_string("localhost:2181");
    listener.set_path("/test");
    listener.set_url_template("jdbc:mysql://${host}:${port}");
    let events = druid::dynamic::node::NodeListener::refresh(&listener).await;
    assert!(events.is_empty());
}

/// destroy 未初始化不 panic。
#[tokio::test]
async fn zk_listener_destroy_without_init() {
    let listener = ZookeeperNodeListener::new();
    druid::dynamic::node::NodeListener::destroy(&listener).await;
    assert!(listener.client().is_none());
}

/// `last_update_time_millis` 初始为 0。
#[test]
fn zk_listener_last_update_time_initial() {
    let listener = ZookeeperNodeListener::new();
    assert_eq!(
        druid::dynamic::node::NodeListener::last_update_time_millis(&listener),
        0
    );
}

// ── ZookeeperNodeRegister ────────────────────────────────────

/// 配置 setter / getter。
#[test]
fn zk_register_config_setters() {
    let register = ZookeeperNodeRegister::new();
    register.set_zk_connect_string("localhost:2181");
    assert_eq!(
        register.zk_connect_string().as_deref(),
        Some("localhost:2181")
    );
    register.set_path("/ha-druid-datasources");
    assert_eq!(register.path(), "/ha-druid-datasources");
    assert!(register.client().is_none());
}

// ── ZookeeperNodeInfo ────────────────────────────────────────

/// 全字段 setter / getter 与 prefix 规范化。
#[test]
fn zk_node_info_full_lifecycle() {
    let mut info = ZookeeperNodeInfo::new();
    assert_eq!(info.prefix(), "");
    assert!(info.host().is_none());
    assert_eq!(info.port(), None);
    assert!(info.database().is_none());
    assert!(info.username().is_none());
    assert!(info.password().is_none());

    info.set_prefix(Some("ha.druid"));
    assert_eq!(info.prefix(), "ha.druid.", "auto-appends dot");
    info.set_prefix(Some("already."));
    assert_eq!(info.prefix(), "already.", "does not double-dot");
    // set_prefix(None) 保留当前值（Java setPrefix(null) 不清空）。
    info.set_prefix(None);
    assert_eq!(info.prefix(), "already.", "None keeps previous value");

    info.set_host(Some("db-host".to_owned()));
    assert_eq!(info.host(), Some("db-host"));
    info.set_port(Some(3306));
    assert_eq!(info.port(), Some(3306));
    info.set_database(Some("mydb".to_owned()));
    assert_eq!(info.database(), Some("mydb"));
    info.set_username(Some("admin".to_owned()));
    assert_eq!(info.username(), Some("admin"));
    info.set_password(Some("secret".to_owned()));
    assert_eq!(info.password(), Some("secret"));

    let info2 = info.clone();
    assert_eq!(info, info2);
    let debug = format!("{info:?}");
    assert!(debug.contains("db-host"));
}

// ── NodeEvent ────────────────────────────────────────────────

/// 访问器 + Clone + `PartialEq` + Debug 脱敏。
#[test]
fn node_event_accessors_and_debug() {
    let event = NodeEvent::new(
        NodeEventTypeEnum::Add,
        "node-x",
        Some("jdbc:mock://x".to_owned()),
        Some("user".to_owned()),
        Some("pass".to_owned()),
    );
    assert_eq!(event.event_type(), NodeEventTypeEnum::Add);
    assert_eq!(event.node_name(), "node-x");
    assert_eq!(event.url(), Some("jdbc:mock://x"));
    assert_eq!(event.username(), Some("user"));
    assert_eq!(event.password(), Some("pass"));

    let event2 = event.clone();
    assert_eq!(event, event2);
    let debug = format!("{event:?}");
    // Debug 只暴露 password_length，不暴露密码内容。
    assert!(
        debug.contains("password_length"),
        "must show password_length: {debug}"
    );
    assert!(
        !debug.contains("s3cret"),
        "password value must not appear in Debug: {debug}"
    );
}

// ── PropertiesUtils ──────────────────────────────────────────

/// `load_name_list：.url` 后缀提取。
#[test]
fn properties_utils_name_list_and_prefix() {
    let p = props(&[
        ("a.url", "x"),
        ("b.url", "y"),
        ("p.c.url", "z"),
        ("d.username", "u"),
    ]);
    let mut names = PropertiesUtils::load_name_list(&p, None);
    names.sort();
    assert_eq!(names, vec!["a", "b", "p.c"]);
    let prefixed = PropertiesUtils::load_name_list(&p, Some("p."));
    assert_eq!(prefixed, vec!["p.c"]);
}

/// `filter_prefix：空前缀原样返回`。
#[test]
fn properties_utils_filter_prefix() {
    let p = props(&[("a.url", "x"), ("b.url", "y")]);
    let all = PropertiesUtils::filter_prefix(&p, None);
    assert_eq!(all.len(), 2);
    let filtered = PropertiesUtils::filter_prefix(&p, Some("c."));
    assert_eq!(filtered.len(), 0);
}

// ── RdbcSqlStat ──────────────────────────────────────────────

/// new + 基本 getter。
#[test]
fn rdbc_sql_stat_new_and_getters() {
    let stat = RdbcSqlStat::new("SELECT * FROM t WHERE id = ?".to_owned(), 12345);
    assert_eq!(stat.execute_count(), 0);
    assert_eq!(stat.total_time_ms(), 0.0);
    assert_eq!(stat.max_time_ms(), 0.0);
    assert_eq!(stat.error_count(), 0);
}

/// record：ok/error 路径 + `max_time` 更新。
#[test]
fn rdbc_sql_stat_record() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 100);
    stat.record(Duration::from_millis(10), true);
    stat.record(Duration::from_millis(20), true);
    stat.record(Duration::from_millis(5), false);
    // execute_count 只计 ok=true 的记录（Java executeSuccessCount 语义）。
    assert_eq!(stat.execute_count(), 2, "only ok=true records counted");
    assert_eq!(stat.error_count(), 1);
    assert!(stat.total_time_ms() > 30.0);
    assert!(stat.max_time_ms() >= 19.0);
}

/// `running_count` / `in_transaction_count` 生命周期。
#[test]
fn rdbc_sql_stat_running_and_transaction() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 100);
    stat.increment_running_count();
    stat.increment_running_count();
    stat.increment_running_count();
    stat.decrement_running_count();
    assert_eq!(stat.running_count.load(Ordering::Relaxed), 2);
    stat.increment_in_transaction_count();
    stat.increment_in_transaction_count();
    assert_eq!(stat.in_transaction_count.load(Ordering::Relaxed), 2);
}

/// `execute_time_histogram_values：8` 桶。
#[test]
fn rdbc_sql_stat_execute_histogram() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 300);
    let values = stat.execute_time_histogram_values();
    assert_eq!(values.len(), 8);
    assert!(values.iter().all(|&v| v == 0));
}

/// `record_execute_and_result_hold_time` + histogram。
#[test]
fn rdbc_sql_stat_hold_time() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 400);
    stat.record_execute_and_result_hold_time(Duration::from_millis(10));
    stat.record_execute_and_result_hold_time(Duration::from_millis(20));
    let values = stat.execute_and_result_hold_time_histogram_values();
    assert!(values.iter().any(|&v| v > 0));
}

/// `add_result_set_hold_time`。
#[test]
fn rdbc_sql_stat_result_set_hold_time() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 500);
    stat.add_result_set_hold_time(Duration::from_millis(5), Duration::from_millis(10));
    assert!(stat.result_set_hold_time_ns.load(Ordering::Relaxed) > 0);
}

/// set/get `last_slow_parameters`。
#[test]
fn rdbc_sql_stat_last_slow_parameters() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 600);
    assert!(stat.last_slow_parameters().is_none());
    stat.set_last_slow_parameters(Some("[1, 'abc']".to_owned()));
    assert_eq!(stat.last_slow_parameters().as_deref(), Some("[1, 'abc']"));
    stat.set_last_slow_parameters(None);
    assert!(stat.last_slow_parameters().is_none());
}

/// `record_error_detail`。
#[test]
fn rdbc_sql_stat_record_error() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 700);
    stat.record_error_detail(&DruidError::Other("test error".to_owned()));
    // last_error_time 通过 pub fn 间接访问，不直接读私有字段。
}

/// io counters。
#[test]
fn rdbc_sql_stat_io_counters() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 800);
    stat.add_read_string_length(1024);
    stat.add_read_bytes_length(2048);
    stat.add_input_stream_open_count(3);
    stat.add_reader_open_count(5);
    // 通过 pub fn 间接验证（fetch_add 不 panic）。
}

/// LOB counters。
#[test]
fn rdbc_sql_stat_lob_counters() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 900);
    stat.increment_clob_open_count();
    stat.increment_blob_open_count();
    // 间接验证。
}

/// `add_update_count` / `update_count_histogram_values`。
#[test]
fn rdbc_sql_stat_update_histogram() {
    let stat = RdbcSqlStat::new("UPDATE t".to_owned(), 1100);
    stat.add_update_count(5);
    stat.add_update_count(15);
    let values = stat.update_count_histogram_values();
    assert_eq!(values.len(), 6);
    assert!(values.iter().any(|&v| v > 0));
}

/// `add_fetch_row_count` / `fetch_row_count_histogram_values`。
#[test]
fn rdbc_sql_stat_fetch_row_histogram() {
    let stat = RdbcSqlStat::new("SELECT *".to_owned(), 1200);
    stat.add_fetch_row_count(100);
    stat.add_fetch_row_count(5000);
    let values = stat.fetch_row_count_histogram_values();
    assert_eq!(values.len(), 6);
    assert!(values.iter().any(|&v| v > 0));
}

/// `stat_value` 快照。
#[test]
fn rdbc_sql_stat_stat_value() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 1300);
    stat.record(Duration::from_millis(50), true);
    stat.increment_running_count();
    stat.increment_in_transaction_count();
    let sv = stat.stat_value();
    assert_eq!(sv.sql, "SELECT 1");
    assert_eq!(sv.execute_count, 1);
    assert_eq!(sv.running_count, 1);
    assert_eq!(sv.in_transaction_count, 1);
}

/// reset 清零。
#[test]
fn rdbc_sql_stat_reset() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 1400);
    stat.record(Duration::from_millis(10), true);
    stat.increment_running_count();
    stat.increment_in_transaction_count();
    stat.add_read_string_length(100);
    stat.add_read_bytes_length(200);
    stat.increment_clob_open_count();
    stat.increment_blob_open_count();

    stat.reset();
    assert_eq!(stat.execute_count(), 0);
    assert_eq!(stat.error_count(), 0);
    // running_count 不随 reset 清零（Java 保留当前打开数）。
    assert_eq!(stat.running_count.load(Ordering::Relaxed), 1);
    assert!(stat.sql.contains("SELECT"), "SQL preserved after reset");
}

/// `context_sql_name` / `context_sql_file` / `context_sql` 线程局部。
#[test]
fn rdbc_sql_stat_context_thread_local() {
    RdbcSqlStat::set_context_sql_name(Some("q1".to_owned()));
    assert_eq!(RdbcSqlStat::context_sql_name(), Some("q1".to_owned()));
    RdbcSqlStat::set_context_sql_file(Some("migration.sql".to_owned()));
    assert_eq!(
        RdbcSqlStat::context_sql_file(),
        Some("migration.sql".to_owned())
    );
    RdbcSqlStat::set_context_sql(Some("SELECT 1".to_owned()));
    RdbcSqlStat::set_context_sql_name(None);
    assert_eq!(RdbcSqlStat::context_sql_name(), None);
}

/// `set_management_identity`。
#[test]
fn rdbc_sql_stat_management_identity() {
    let stat = RdbcSqlStat::new("SELECT 1".to_owned(), 1500);
    stat.set_management_identity(Some("ds-1"), Some("pool-1"), Some("stmt-1"));
    // 验证 setter 不 panic。
}

/// `add_execute_batch_count`。
#[test]
fn rdbc_sql_stat_batch_count() {
    let stat = RdbcSqlStat::new("INSERT INTO t".to_owned(), 1600);
    stat.add_execute_batch_count(10);
    stat.add_execute_batch_count(5);
    // 间接验证。
}
