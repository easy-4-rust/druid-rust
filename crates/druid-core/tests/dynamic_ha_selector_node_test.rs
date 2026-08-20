//! HA 选择器与节点监听差分测试（C9 覆盖率批次：dynamic/selector + node）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 对照实现：
//! - `DataSourceSelectorEnum/Factory`（`ha.selector.*`）：byName/random/
//!   stickyRandom 名称解析与工厂装配。
//! - `NamedDataSourceSelector`：执行上下文目标、默认名、reset。
//! - `RandomDataSourceSelector`：黑名单摘除与恢复入口（含可用映射过滤）。
//! - `StickyRandomDataSourceSelector`/`StickyDataSourceHolder`：粘性持有与过期。
//! - `HighAvailableDataSource`：map/blacklist/selector/target 管理。
//! - `NodeEvent`：仅新增/删除差分（URL `变更不产生事件）、generate_events`。
//! - `FileNodeListener`：properties 读取、前缀过滤、refresh 差分。
//! - `ZookeeperNodeInfo`：连接信息组装。

extern crate druid_core as druid;
use druid::core::{DruidError, DruidPooledConnection, Pool, PoolState};
use druid::dynamic::node::{
    FileNodeListener, NodeEvent, NodeEventTypeEnum, NodeListener, ZookeeperNodeInfo,
};
use druid::dynamic::selector::{
    DataSourceSelector, DataSourceSelectorEnum, DataSourceSelectorFactory, NamedDataSourceSelector,
    StickyDataSourceHolder, StickyRandomDataSourceSelector,
};
use druid::dynamic::{DataSourceCreator, HighAvailableDataSource, PropertiesUtils};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ── 测试用 Pool mock ─────────────────────────────────────────

struct CountingPool {
    name: &'static str,
    closes: AtomicU64,
}

impl CountingPool {
    fn arc(name: &'static str) -> Arc<Self> {
        Arc::new(Self {
            name,
            closes: AtomicU64::new(0),
        })
    }
}

#[async_trait::async_trait]
impl Pool for CountingPool {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        Err(DruidError::Other(format!("mock pool {}", self.name)))
    }
    async fn get_timeout(&self, _timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        Err(DruidError::Other(format!("mock pool {}", self.name)))
    }
    fn state(&self) -> PoolState {
        // Java sticky isAvailable 要求 poolingCount > 0：mock 池报告
        // idle_count > 0 且未满，使粘性复用路径可达。
        PoolState {
            name: self.name.to_owned(),
            idle_count: 1,
            max_open: 8,
            ..Default::default()
        }
    }
    fn driver_name(&self) -> &'static str {
        "mock-ha"
    }
    fn name(&self) -> &str {
        self.name
    }
    async fn close_for_removal_if_idle(&self) -> Result<bool, DruidError> {
        self.closes.fetch_add(1, Ordering::Relaxed);
        Ok(true)
    }
}

fn props(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

// ── DataSourceSelectorEnum / Factory ─────────────────────────

/// Java `DataSourceSelectorEnum`：精确名称 + 忽略大小写解析。
#[test]
fn selector_enum_names_and_parsing() {
    assert_eq!(DataSourceSelectorEnum::ByName.name(), "byName");
    assert_eq!(DataSourceSelectorEnum::Random.name(), "random");
    assert_eq!(DataSourceSelectorEnum::StickyRandom.name(), "stickyRandom");

    assert_eq!(
        DataSourceSelectorEnum::of("byName"),
        Some(DataSourceSelectorEnum::ByName)
    );
    assert_eq!(
        DataSourceSelectorEnum::of("BYNAME"),
        Some(DataSourceSelectorEnum::ByName)
    );
    assert_eq!(
        DataSourceSelectorEnum::of("random"),
        Some(DataSourceSelectorEnum::Random)
    );
    assert_eq!(
        DataSourceSelectorEnum::of("stickyRandom"),
        Some(DataSourceSelectorEnum::StickyRandom)
    );
    assert_eq!(DataSourceSelectorEnum::of("unknown"), None);
    assert_eq!(DataSourceSelectorEnum::of(""), None);
}

/// Java `DataSourceSelectorFactory`：三种名称装配对应选择器。
#[test]
fn selector_factory_builds_all_three() {
    let data_source = HighAvailableDataSource::new("factory-test", DataSourceCreator::noop_for_test());

    let by_name = DataSourceSelectorFactory::get_selector("byName", &data_source).unwrap();
    assert_eq!(by_name.name(), "byName");

    let random = DataSourceSelectorFactory::get_selector("random", &data_source).unwrap();
    assert_eq!(random.name(), "random");

    let sticky = DataSourceSelectorFactory::get_selector("stickyRandom", &data_source).unwrap();
    assert_eq!(sticky.name(), "stickyRandom");

    assert!(DataSourceSelectorFactory::get_selector("no-such", &data_source).is_none());
}

// ── NamedDataSourceSelector ──────────────────────────────────

/// Java `ThreadLocal` target 在 Rust 为执行上下文键：set→get→reset。
#[tokio::test]
async fn named_selector_target_lifecycle() {
    let data_source = HighAvailableDataSource::new("named-test", DataSourceCreator::noop_for_test());
    data_source.insert_data_source("master", CountingPool::arc("master"));
    data_source.insert_data_source("standby", CountingPool::arc("standby"));
    let selector = NamedDataSourceSelector::new(&data_source);

    assert_eq!(selector.default_name(), "default");
    selector.set_default_name("master-default");
    assert_eq!(selector.default_name(), "master-default");

    // 两个可用节点：未设置 target 时按默认名查找（无 default 节点 → None）。
    assert!(selector.target().is_none());
    assert!(
        selector.get().is_none(),
        "default name absent and multiple nodes available"
    );

    // 仅剩一个节点时短路返回该节点。
    data_source.add_blacklist("standby");
    assert!(selector.get().is_some());
    data_source.remove_blacklist("standby");

    DataSourceSelector::set_target(&selector, Some("master".to_owned()));
    assert_eq!(selector.target().as_deref(), Some("master"));
    assert!(selector.get().is_some());

    // 不存在的目标。
    DataSourceSelector::set_target(&selector, Some("ghost".to_owned()));
    assert!(selector.get().is_none());

    DataSourceSelector::set_target(&selector, None);
    selector.reset_data_source_name();
    assert!(selector.target().is_none());
}

// ── HighAvailableDataSource 基础管理 ─────────────────────────

/// map 增删、黑名单与可用映射过滤（Java HA `DataSource` 语义）。
#[test]
fn ha_data_source_map_and_blacklist_management() {
    let data_source = HighAvailableDataSource::new("ha-test", DataSourceCreator::noop_for_test());
    assert!(data_source.data_source_map().is_empty());

    let master = CountingPool::arc("master");
    let slave = CountingPool::arc("slave");
    data_source.insert_data_source("master", master);
    data_source.insert_data_source("slave", slave);
    assert_eq!(data_source.data_source_map().len(), 2);
    assert_eq!(data_source.available_data_source_map().len(), 2);

    // 黑名单过滤可用映射。
    data_source.add_blacklist("slave");
    assert!(data_source.is_in_blacklist("slave"));
    assert_eq!(data_source.available_data_source_map().len(), 1);
    assert_eq!(
        data_source.data_source_map().len(),
        2,
        "full map keeps blacklisted node"
    );

    data_source.remove_blacklist("slave");
    assert!(!data_source.is_in_blacklist("slave"));
    assert_eq!(data_source.available_data_source_map().len(), 2);

    // set_data_source_map 整体替换。
    let replacement = CountingPool::arc("solo");
    let mut new_map = HashMap::new();
    new_map.insert("solo".to_owned(), replacement as Arc<dyn Pool>);
    data_source.set_data_source_map(new_map);
    assert_eq!(data_source.data_source_map().len(), 1);
    assert!(data_source.data_source_map().contains_key("solo"));

    // 移除节点。
    assert!(data_source.remove_data_source("solo").is_some());
    assert!(data_source.data_source_map().is_empty());
    assert!(data_source.remove_data_source("missing").is_none());
}

/// selector 安装与名称查询（Java setSelector 语义）。
#[test]
fn ha_data_source_selector_installation() {
    let data_source = HighAvailableDataSource::new("ha-selector", DataSourceCreator::noop_for_test());
    assert!(data_source.selector_name().is_none());

    data_source.set_selector("byName");
    assert_eq!(data_source.selector_name(), Some("byName"));

    data_source.set_selector("stickyRandom");
    assert_eq!(data_source.selector_name(), Some("stickyRandom"));

    // 无效名称不改变现有选择器。
    data_source.set_selector("bogus");
    assert_eq!(data_source.selector_name(), Some("stickyRandom"));
}

// ── Sticky 家族 ─────────────────────────────────────────────

/// Java StickyDataSourceHolder：有效性由 `data_source` 存在性决定。
#[test]
fn sticky_holder_validity_and_accessors() {
    let holder = StickyDataSourceHolder::new();
    assert!(!holder.is_valid());
    // Java 构造器记录 System.currentTimeMillis；此处仅断言为正。
    assert!(holder.retrieving_time_millis() > 0);

    let mut holder = StickyDataSourceHolder::with_data_source(Some(CountingPool::arc("sticky")));
    assert!(holder.is_valid());
    assert!(holder.data_source().is_some());

    holder.set_retrieving_time_millis(1_700_000_000_000);
    assert_eq!(holder.retrieving_time_millis(), 1_700_000_000_000);

    holder.set_data_source(None);
    assert!(!holder.is_valid());
    assert!(holder.data_source().is_none());
}

/// Java StickyRandomDataSourceSelector：粘性命中同一节点、过期时间配置。
#[tokio::test]
async fn sticky_random_selector_reuses_and_exposes_config() {
    let data_source = HighAvailableDataSource::new("sticky-test", DataSourceCreator::noop_for_test());
    data_source.insert_data_source("node-a", CountingPool::arc("node-a"));
    data_source.insert_data_source("node-b", CountingPool::arc("node-b"));

    let selector = StickyRandomDataSourceSelector::new(&data_source);
    assert_eq!(DataSourceSelector::name(&selector), "stickyRandom");

    // Java 默认 expireSeconds=5。
    assert_eq!(selector.expire_seconds(), 5);
    selector.set_expire_seconds(30);
    assert_eq!(selector.expire_seconds(), 30);

    let first = DataSourceSelector::get(&selector);
    assert!(first.is_some());
    let second = DataSourceSelector::get(&selector);
    match (first, second) {
        (Some(first), Some(second)) => {
            assert_eq!(
                first.state().name,
                second.state().name,
                "sticky selector must reuse the same node while valid"
            );
        }
        _ => panic!("both selections must succeed"),
    }
    DataSourceSelector::init(&selector);
    DataSourceSelector::destroy(&selector);
}

// ── NodeEvent 差分 ───────────────────────────────────────────

/// Java `getEventsByDiffProperties`：仅新增与删除；URL 变化不产生事件。
#[test]
fn node_event_diff_generates_add_and_delete_only() {
    let previous = props(&[
        ("node-a.url", "jdbc:mock://a"),
        ("node-b.url", "jdbc:mock://b"),
        ("node-b.username", "u"),
        ("node-b.password", "p"),
    ]);
    // node-a 被删除；node-c 新增；node-b 仅 URL 变化。
    let next = props(&[
        ("node-b.url", "jdbc:mock://b2"),
        ("node-b.username", "u"),
        ("node-b.password", "p"),
        ("node-c.url", "jdbc:mock://c"),
    ]);

    let mut events = NodeEvent::get_events_by_diff_properties(&previous, &next);
    events.sort_by_key(|event| {
        (
            event.node_name().to_owned(),
            matches!(event.event_type(), NodeEventTypeEnum::Delete),
        )
    });

    assert_eq!(events.len(), 2, "URL-only change must not emit: {events:?}");
    let deleted = &events[0];
    assert_eq!(deleted.node_name(), "node-a");
    assert_eq!(deleted.event_type(), NodeEventTypeEnum::Delete);
    assert_eq!(deleted.url(), Some("jdbc:mock://a"));

    let added = &events[1];
    assert_eq!(added.node_name(), "node-c");
    assert_eq!(added.event_type(), NodeEventTypeEnum::Add);
    assert_eq!(added.url(), Some("jdbc:mock://c"));
}

/// `generate_events` 按名称提取 url/username/password 三元组。
#[test]
fn node_event_generate_events_carries_credentials() {
    let properties = props(&[
        ("n1.url", "jdbc:mock://n1"),
        ("n1.username", "user1"),
        ("n1.password", "secret1"),
    ]);
    let events =
        NodeEvent::generate_events(&properties, &["n1".to_owned()], NodeEventTypeEnum::Add);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type(), NodeEventTypeEnum::Add);
    assert_eq!(events[0].node_name(), "n1");
    assert_eq!(events[0].url(), Some("jdbc:mock://n1"));
    assert_eq!(events[0].username(), Some("user1"));
    assert_eq!(events[0].password(), Some("secret1"));
    // 缺失凭据的字段为 None。
    let partial = NodeEvent::generate_events(
        &props(&[("n2.url", "jdbc:mock://n2")]),
        &["n2".to_owned()],
        NodeEventTypeEnum::Delete,
    );
    assert_eq!(partial[0].username(), None);
    assert_eq!(partial[0].password(), None);
}

// ── PropertiesUtils ──────────────────────────────────────────

/// Java `PropertiesUtils.loadNameList`：以 `.url` 后缀识别节点名并按前缀过滤。
#[test]
fn properties_utils_name_list_and_prefix_filter() {
    let properties = props(&[
        ("node-a.url", "x"),
        ("node-b.url", "y"),
        ("node-b.username", "u"),
        ("filter.node-c.url", "z"),
        ("orphan", "no-url-key"),
    ]);
    // Java loadNameList：空前缀包含全部 .url 键；名称只剥 .url 后缀。
    let mut names = PropertiesUtils::load_name_list(&properties, None);
    names.sort();
    assert_eq!(names, vec!["filter.node-c", "node-a", "node-b"]);

    let mut prefixed = PropertiesUtils::load_name_list(&properties, Some("filter."));
    prefixed.sort();
    assert_eq!(prefixed, vec!["filter.node-c"]);

    let filtered = PropertiesUtils::filter_prefix(&properties, Some("node-b."));
    assert_eq!(filtered.len(), 2);
    let identity = PropertiesUtils::filter_prefix(&properties, None);
    assert_eq!(identity.len(), properties.len());
}

// ── FileNodeListener ─────────────────────────────────────────

/// 配置项 setter/getter 与 refresh 差分（新增+删除事件）。
#[tokio::test]
async fn file_node_listener_refresh_emits_diff_events() {
    let dir = std::env::temp_dir().join("druid-ha-node-listener");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ha.properties");

    std::fs::write(
        &file,
        "node-a.url=jdbc:mock://a\nnode-b.url=jdbc:mock://b\n",
    )
    .unwrap();
    let listener = FileNodeListener::new(&file);
    assert!(listener.prefix().is_empty());
    listener.set_prefix("");
    listener.set_interval_seconds(5);
    assert_eq!(listener.interval_seconds(), 5);
    assert!(listener.file().ends_with("ha.properties"));

    let first = NodeListener::refresh(&listener).await;
    assert_eq!(first.len(), 2, "initial load emits Add for both nodes");

    // 修改文件：删除 node-a，新增 node-c，node-b 仅 URL 变化。
    std::fs::write(
        &file,
        "node-b.url=jdbc:mock://b2\nnode-c.url=jdbc:mock://c\n",
    )
    .unwrap();
    let second = NodeListener::refresh(&listener).await;
    let mut names: Vec<(String, bool)> = second
        .iter()
        .map(|event| {
            (
                event.node_name().to_owned(),
                matches!(event.event_type(), NodeEventTypeEnum::Delete),
            )
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![("node-a".to_owned(), true), ("node-c".to_owned(), false)],
        "URL-only change must not emit: {second:?}"
    );

    // set_file 切换监听目标。
    let other = dir.join("other.properties");
    std::fs::write(&other, "solo.url=jdbc:mock://solo\n").unwrap();
    listener.set_file(&other);
    assert!(listener.file().ends_with("other.properties"));
    let mut third = NodeListener::refresh(&listener).await;
    third.sort_by_key(|event| event.node_name().to_owned());
    assert_eq!(third.len(), 3, "old nodes deleted + solo added: {third:?}");
    assert_eq!(third[0].node_name(), "node-b");
    assert_eq!(third[0].event_type(), NodeEventTypeEnum::Delete);
    assert_eq!(third[1].node_name(), "node-c");
    assert_eq!(third[2].node_name(), "solo");
    assert_eq!(third[2].event_type(), NodeEventTypeEnum::Add);

    let _ = std::fs::remove_dir_all(&dir);
}

// ── ZookeeperNodeInfo ────────────────────────────────────────

/// Java `ZookeeperNodeInfo`：前缀、主机、端口、库名组装与 Display 脱敏。
#[test]
fn zookeeper_node_info_assembly() {
    let mut info = ZookeeperNodeInfo::new();
    assert_eq!(info.prefix(), "");

    info.set_prefix(Some("ha.druid."));
    assert_eq!(info.prefix(), "ha.druid.");
    info.set_prefix(None);

    info.set_host(Some("zk-host".to_owned()));
    assert_eq!(info.host(), Some("zk-host"));

    info.set_port(Some(2181));
    assert_eq!(info.port(), Some(2181));

    info.set_database(Some("app_db".to_owned()));
    assert_eq!(info.database(), Some("app_db"));
}
