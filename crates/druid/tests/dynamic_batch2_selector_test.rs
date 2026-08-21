//! Second batch selector coverage tests for the `dynamic` module.
//!
//! Targets uncovered lines in `RandomDataSourceSelector`,
//! `NamedDataSourceSelector`, `StickyRandomDataSourceSelector`,
//! `DataSourceSelectorEnum`, `DataSourceSelectorFactory`,
//! `StickyDataSourceHolder`, `RandomDataSourceRecoverTask`,
//! `RandomDataSourceValidateFilter`, `DynamicDataSource`,
//! `LoadBalancer`, `DataSourceGroup`, `SqlHint`.

use druid::core::{DruidError, DruidPooledConnection, Pool, PoolState};
use druid::dynamic::selector::{
    DataSourceSelector, NamedDataSourceSelector, RandomDataSourceSelector,
    RandomDataSourceValidateFilter, StickyDataSourceHolder, StickyRandomDataSourceSelector,
};
use druid::dynamic::{
    DataSourceCreator, DynamicDataSource, HighAvailableDataSource, LoadBalancer, SqlHint,
};
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

// ===========================================================================
// RandomDataSourceSelector
// ===========================================================================

#[test]
fn random_selector_empty_candidates() {
    let ha = HighAvailableDataSource::new("empty-cand", DataSourceCreator::noop_for_test());
    let selector = RandomDataSourceSelector::new(&ha);
    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_none());
}

#[test]
fn random_selector_single_candidate() {
    let ha = HighAvailableDataSource::new("single-cand", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = RandomDataSourceSelector::new(&ha);
    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_some());
    assert_eq!(pool.unwrap().name(), "p1");
}

#[test]
fn random_selector_blacklist_filter() {
    let ha = HighAvailableDataSource::new("bl-filter", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    ha.insert_data_source("p2", MockPool::arc("p2"));
    let selector = RandomDataSourceSelector::new(&ha);

    let p1 = ha.data_source_map()["p1"].clone();
    selector.add_blacklist(p1);

    for _ in 0..10 {
        let pool = DataSourceSelector::get(&selector);
        assert!(pool.is_some());
        assert_eq!(pool.unwrap().name(), "p2");
    }
}

#[test]
fn random_selector_all_blacklisted_fallback() {
    let ha = HighAvailableDataSource::new("all-bl", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = RandomDataSourceSelector::new(&ha);

    let p1 = ha.data_source_map()["p1"].clone();
    selector.add_blacklist(p1);

    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_some());
}

#[test]
fn random_selector_blacklist_snapshot() {
    let ha = HighAvailableDataSource::new("bl-snap", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = RandomDataSourceSelector::new(&ha);

    assert!(selector.blacklist().is_empty());
    let p1 = ha.data_source_map()["p1"].clone();
    selector.add_blacklist(p1.clone());
    assert_eq!(selector.blacklist().len(), 1);
    assert!(selector.contains_in_blacklist(&p1));

    selector.remove_blacklist(&p1);
    assert!(selector.blacklist().is_empty());
}

#[test]
fn random_selector_map_types() {
    let ha = HighAvailableDataSource::new("map-types", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    ha.insert_data_source("p2", MockPool::arc("p2"));
    ha.add_blacklist("p2");

    let selector = RandomDataSourceSelector::new(&ha);
    assert_eq!(selector.data_source_map().len(), 1);
    assert_eq!(selector.full_data_source_map().len(), 2);
}

#[test]
fn random_selector_checking_interval() {
    let ha = HighAvailableDataSource::new("ci-test", DataSourceCreator::noop_for_test());
    let selector = RandomDataSourceSelector::new(&ha);
    assert_eq!(selector.checking_interval_seconds(), 10);
    selector.set_checking_interval_seconds(30);
    assert_eq!(selector.checking_interval_seconds(), 30);
}

#[test]
fn random_selector_recovery_interval() {
    let ha = HighAvailableDataSource::new("ri-test", DataSourceCreator::noop_for_test());
    let selector = RandomDataSourceSelector::new(&ha);
    assert_eq!(selector.recovery_interval_seconds(), 120);
    selector.set_recovery_interval_seconds(60);
    assert_eq!(selector.recovery_interval_seconds(), 60);
}

#[test]
fn random_selector_validation_sleep() {
    let ha = HighAvailableDataSource::new("vs-test", DataSourceCreator::noop_for_test());
    let selector = RandomDataSourceSelector::new(&ha);
    assert_eq!(selector.validation_sleep_seconds(), 0);
    selector.set_validation_sleep_seconds(5);
    assert_eq!(selector.validation_sleep_seconds(), 5);
}

#[test]
fn random_selector_blacklist_threshold() {
    let ha = HighAvailableDataSource::new("bt-test", DataSourceCreator::noop_for_test());
    let selector = RandomDataSourceSelector::new(&ha);
    assert_eq!(selector.blacklist_threshold(), 3);
    selector.set_blacklist_threshold(5);
    assert_eq!(selector.blacklist_threshold(), 5);
}

#[test]
fn random_selector_set_target_noop() {
    let ha = HighAvailableDataSource::new("rt-noop", DataSourceCreator::noop_for_test());
    let selector = RandomDataSourceSelector::new(&ha);
    DataSourceSelector::set_target(&selector, Some("target".to_owned()));
}

#[test]
fn random_selector_name() {
    let ha = HighAvailableDataSource::new("rname", DataSourceCreator::noop_for_test());
    let selector = RandomDataSourceSelector::new(&ha);
    assert_eq!(DataSourceSelector::name(&selector), "random");
}

#[test]
fn random_selector_init_test_on_borrow_skips() {
    let ha = HighAvailableDataSource::new("tob-init", DataSourceCreator::noop_for_test());
    ha.set_test_on_borrow(true);
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = RandomDataSourceSelector::new(&ha);
    DataSourceSelector::init(&selector);
}

#[test]
fn random_selector_init_test_on_return_skips() {
    let ha = HighAvailableDataSource::new("tor-init", DataSourceCreator::noop_for_test());
    ha.set_test_on_return(true);
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = RandomDataSourceSelector::new(&ha);
    DataSourceSelector::init(&selector);
}

#[test]
fn random_selector_destroy() {
    let ha = HighAvailableDataSource::new("rdestroy", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = RandomDataSourceSelector::new(&ha);
    DataSourceSelector::init(&selector);
    DataSourceSelector::destroy(&selector);
}

#[test]
fn random_selector_init_no_runtime() {
    let ha = HighAvailableDataSource::new("no-rt", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = RandomDataSourceSelector::new(&ha);
    DataSourceSelector::init(&selector);
}

#[test]
fn random_selector_candidates_filter_busy() {
    let ha = HighAvailableDataSource::new("busy-filter", DataSourceCreator::noop_for_test());
    ha.insert_data_source("busy", MockPool::arc_custom("busy", 0, 8));
    ha.insert_data_source("idle", MockPool::arc_custom("idle", 1, 8));
    let selector = RandomDataSourceSelector::new(&ha);

    for _ in 0..10 {
        let pool = DataSourceSelector::get(&selector);
        assert!(pool.is_some());
        assert_eq!(pool.unwrap().name(), "idle");
    }
}

#[test]
fn random_selector_all_busy_returns_any() {
    let ha = HighAvailableDataSource::new("all-busy", DataSourceCreator::noop_for_test());
    ha.insert_data_source("b1", MockPool::arc_custom("b1", 0, 8));
    ha.insert_data_source("b2", MockPool::arc_custom("b2", 0, 8));
    let selector = RandomDataSourceSelector::new(&ha);

    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_some());
}

#[test]
fn random_selector_blacklist_ge_map_fallback() {
    let ha = HighAvailableDataSource::new("bl-ge", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = RandomDataSourceSelector::new(&ha);

    let p1 = ha.data_source_map()["p1"].clone();
    selector.add_blacklist(p1);
    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_some());
}

// ===========================================================================
// NamedDataSourceSelector
// ===========================================================================

#[test]
fn named_selector_set_and_get() {
    let ha = HighAvailableDataSource::new("named-get", DataSourceCreator::noop_for_test());
    ha.insert_data_source("master", MockPool::arc("master"));
    ha.insert_data_source("slave", MockPool::arc("slave"));
    let selector = NamedDataSourceSelector::new(&ha);

    DataSourceSelector::set_target(&selector, Some("master".to_owned()));
    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_some());
    assert_eq!(pool.unwrap().name(), "master");
}

#[test]
fn named_selector_default_name_fallback() {
    let ha = HighAvailableDataSource::new("named-default", DataSourceCreator::noop_for_test());
    ha.insert_data_source("master", MockPool::arc("master"));
    let selector = NamedDataSourceSelector::new(&ha);

    selector.set_default_name("master");
    DataSourceSelector::set_target(&selector, None);
    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_some());
    assert_eq!(pool.unwrap().name(), "master");
}

#[test]
fn named_selector_wrong_default() {
    let ha = HighAvailableDataSource::new("named-wrong", DataSourceCreator::noop_for_test());
    ha.insert_data_source("master", MockPool::arc("master"));
    ha.insert_data_source("slave", MockPool::arc("slave"));
    let selector = NamedDataSourceSelector::new(&ha);

    selector.set_default_name("nonexistent");
    DataSourceSelector::set_target(&selector, None);
    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_none(), "default name not in map => None");
}

#[test]
fn named_selector_single_node() {
    let ha = HighAvailableDataSource::new("named-single", DataSourceCreator::noop_for_test());
    ha.insert_data_source("only", MockPool::arc("only"));
    let selector = NamedDataSourceSelector::new(&ha);

    DataSourceSelector::set_target(&selector, Some("ghost".to_owned()));
    let pool = DataSourceSelector::get(&selector);
    assert!(pool.is_some());
    assert_eq!(pool.unwrap().name(), "only");
}

#[test]
fn named_selector_empty() {
    let ha = HighAvailableDataSource::new("named-empty", DataSourceCreator::noop_for_test());
    let selector = NamedDataSourceSelector::new(&ha);
    assert!(DataSourceSelector::get(&selector).is_none());
}

#[test]
fn named_selector_reset() {
    let ha = HighAvailableDataSource::new("named-reset", DataSourceCreator::noop_for_test());
    let selector = NamedDataSourceSelector::new(&ha);

    DataSourceSelector::set_target(&selector, Some("target".to_owned()));
    assert!(selector.target().is_some());
    selector.reset_data_source_name();
    assert!(selector.target().is_none());
}

#[test]
fn named_selector_name() {
    let ha = HighAvailableDataSource::new("named-name", DataSourceCreator::noop_for_test());
    let selector = NamedDataSourceSelector::new(&ha);
    assert_eq!(DataSourceSelector::name(&selector), "byName");
}

#[test]
fn named_selector_init_noop() {
    let ha = HighAvailableDataSource::new("named-init", DataSourceCreator::noop_for_test());
    let selector = NamedDataSourceSelector::new(&ha);
    DataSourceSelector::init(&selector);
}

#[test]
fn named_selector_destroy() {
    let ha = HighAvailableDataSource::new("named-destroy", DataSourceCreator::noop_for_test());
    let selector = NamedDataSourceSelector::new(&ha);
    DataSourceSelector::set_target(&selector, Some("t".to_owned()));
    DataSourceSelector::destroy(&selector);
    assert!(selector.target().is_none());
}

#[test]
fn named_selector_ha_dropped() {
    let selector = {
        let ha = HighAvailableDataSource::new("named-dropped", DataSourceCreator::noop_for_test());
        ha.insert_data_source("p1", MockPool::arc("p1"));
        NamedDataSourceSelector::new(&ha)
    };
    assert!(DataSourceSelector::get(&selector).is_none());
}

// ===========================================================================
// StickyRandomDataSourceSelector
// ===========================================================================

#[tokio::test]
async fn sticky_selector_reuses_pool() {
    let ha = HighAvailableDataSource::new("sticky-reuse", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    ha.insert_data_source("p2", MockPool::arc("p2"));
    let selector = StickyRandomDataSourceSelector::new(&ha);

    let first = DataSourceSelector::get(&selector);
    let second = DataSourceSelector::get(&selector);
    assert!(first.is_some());
    assert!(second.is_some());
    assert_eq!(
        first.unwrap().name(),
        second.unwrap().name(),
        "sticky must return same pool"
    );
}

#[test]
fn sticky_selector_expire_config() {
    let ha = HighAvailableDataSource::new("sticky-expire", DataSourceCreator::noop_for_test());
    let selector = StickyRandomDataSourceSelector::new(&ha);
    assert_eq!(selector.expire_seconds(), 5);
    selector.set_expire_seconds(60);
    assert_eq!(selector.expire_seconds(), 60);
}

#[test]
fn sticky_selector_name() {
    let ha = HighAvailableDataSource::new("sticky-name", DataSourceCreator::noop_for_test());
    let selector = StickyRandomDataSourceSelector::new(&ha);
    assert_eq!(DataSourceSelector::name(&selector), "stickyRandom");
}

#[test]
fn sticky_selector_set_target_noop() {
    let ha = HighAvailableDataSource::new("sticky-target", DataSourceCreator::noop_for_test());
    let selector = StickyRandomDataSourceSelector::new(&ha);
    DataSourceSelector::set_target(&selector, Some("t".to_owned()));
}

#[test]
fn sticky_selector_init() {
    let ha = HighAvailableDataSource::new("sticky-init", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = StickyRandomDataSourceSelector::new(&ha);
    DataSourceSelector::init(&selector);
}

#[test]
fn sticky_selector_destroy() {
    let ha = HighAvailableDataSource::new("sticky-destroy", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = StickyRandomDataSourceSelector::new(&ha);
    DataSourceSelector::get(&selector);
    DataSourceSelector::destroy(&selector);
}

#[test]
fn sticky_selector_random_accessor() {
    let ha = HighAvailableDataSource::new("sticky-accessor", DataSourceCreator::noop_for_test());
    let selector = StickyRandomDataSourceSelector::new(&ha);
    let _random = selector.random_selector();
}

#[test]
fn sticky_selector_empty() {
    let ha = HighAvailableDataSource::new("sticky-empty", DataSourceCreator::noop_for_test());
    let selector = StickyRandomDataSourceSelector::new(&ha);
    assert!(DataSourceSelector::get(&selector).is_none());
}

#[test]
fn sticky_selector_ha_dropped() {
    let selector = {
        let ha = HighAvailableDataSource::new("sticky-dropped", DataSourceCreator::noop_for_test());
        ha.insert_data_source("p1", MockPool::arc("p1"));
        StickyRandomDataSourceSelector::new(&ha)
    };
    assert!(DataSourceSelector::get(&selector).is_none());
}

#[test]
fn sticky_selector_expired_holder() {
    let ha = HighAvailableDataSource::new("sticky-expired", DataSourceCreator::noop_for_test());
    ha.insert_data_source("p1", MockPool::arc("p1"));
    let selector = StickyRandomDataSourceSelector::new(&ha);

    selector.set_expire_seconds(0);

    let first = DataSourceSelector::get(&selector);
    assert!(first.is_some());
    let second = DataSourceSelector::get(&selector);
    assert!(second.is_some());
}

// ===========================================================================
// DataSourceSelectorEnum and Factory
// ===========================================================================

#[test]
fn selector_enum_case_insensitive() {
    assert_eq!(
        druid::dynamic::DataSourceSelectorEnum::of("BYNAME"),
        Some(druid::dynamic::DataSourceSelectorEnum::ByName)
    );
    assert_eq!(
        druid::dynamic::DataSourceSelectorEnum::of("RANDOM"),
        Some(druid::dynamic::DataSourceSelectorEnum::Random)
    );
    assert_eq!(
        druid::dynamic::DataSourceSelectorEnum::of("STICKYRANDOM"),
        Some(druid::dynamic::DataSourceSelectorEnum::StickyRandom)
    );
}

#[test]
fn selector_enum_empty() {
    assert!(druid::dynamic::DataSourceSelectorEnum::of("").is_none());
}

#[test]
fn selector_enum_all_names() {
    assert_eq!(
        druid::dynamic::DataSourceSelectorEnum::ByName.name(),
        "byName"
    );
    assert_eq!(
        druid::dynamic::DataSourceSelectorEnum::Random.name(),
        "random"
    );
    assert_eq!(
        druid::dynamic::DataSourceSelectorEnum::StickyRandom.name(),
        "stickyRandom"
    );
}

#[test]
fn factory_all_valid_names() {
    let ha = HighAvailableDataSource::new("factory-all", DataSourceCreator::noop_for_test());
    assert!(druid::dynamic::DataSourceSelectorFactory::get_selector("byName", &ha).is_some());
    assert!(druid::dynamic::DataSourceSelectorFactory::get_selector("random", &ha).is_some());
    assert!(druid::dynamic::DataSourceSelectorFactory::get_selector("stickyRandom", &ha).is_some());
}

#[test]
fn factory_invalid_name() {
    let ha = HighAvailableDataSource::new("factory-invalid", DataSourceCreator::noop_for_test());
    assert!(druid::dynamic::DataSourceSelectorFactory::get_selector("invalid", &ha).is_none());
}

// ===========================================================================
// StickyDataSourceHolder
// ===========================================================================

#[test]
fn holder_new_invalid() {
    let holder = StickyDataSourceHolder::new();
    assert!(!holder.is_valid());
    assert!(holder.data_source().is_none());
    assert!(holder.retrieving_time_millis() > 0);
}

#[test]
fn holder_with_none() {
    let holder = StickyDataSourceHolder::with_data_source(None);
    assert!(!holder.is_valid());
}

#[test]
fn holder_with_some() {
    let holder = StickyDataSourceHolder::with_data_source(Some(MockPool::arc("h")));
    assert!(holder.is_valid());
    assert!(holder.data_source().is_some());
}

#[test]
fn holder_set_time() {
    let mut holder = StickyDataSourceHolder::new();
    holder.set_retrieving_time_millis(12345);
    assert_eq!(holder.retrieving_time_millis(), 12345);
}

#[test]
fn holder_set_none_invalidates() {
    let mut holder = StickyDataSourceHolder::with_data_source(Some(MockPool::arc("h")));
    assert!(holder.is_valid());
    holder.set_data_source(None);
    assert!(!holder.is_valid());
}

#[test]
fn holder_default() {
    let holder = StickyDataSourceHolder::default();
    assert!(!holder.is_valid());
}

#[test]
fn holder_clone() {
    let holder = StickyDataSourceHolder::with_data_source(Some(MockPool::arc("c")));
    let cloned = holder.clone();
    assert!(cloned.is_valid());
}

// ===========================================================================
// RandomDataSourceRecoverTask / ValidateFilter
// ===========================================================================

#[test]
fn recover_task_default_interval() {
    assert_eq!(
        druid::dynamic::RandomDataSourceRecoverTask::DEFAULT_RECOVER_INTERVAL_SECONDS,
        120
    );
}

#[test]
fn validate_filter_name() {
    use druid::core::{AfterFilter, BeforeFilter};
    let filter = RandomDataSourceValidateFilter;
    assert_eq!(BeforeFilter::name(&filter), "randomDataSourceValidate");
    assert_eq!(AfterFilter::name(&filter), "randomDataSourceValidate");
}

// ===========================================================================
// DynamicDataSource
// ===========================================================================

#[test]
fn dynamic_route_write() {
    let master = MockPool::arc("master");
    let group = druid::dynamic::DataSourceGroup::new(
        "g1",
        master,
        vec![],
        Arc::new(druid::dynamic::RoundRobinBalancer::new()),
    );
    let ds = DynamicDataSource::new(group);
    let pool = ds.route(SqlHint::Write);
    assert_eq!(pool.name(), "master");
}

#[test]
fn dynamic_route_read_fallback() {
    let master = MockPool::arc("master");
    let group = druid::dynamic::DataSourceGroup::new(
        "g2",
        master,
        vec![],
        Arc::new(druid::dynamic::RoundRobinBalancer::new()),
    );
    let ds = DynamicDataSource::new(group);
    let pool = ds.route(SqlHint::Read);
    assert_eq!(pool.name(), "master");
}

#[test]
fn dynamic_route_auto() {
    let master = MockPool::arc("master");
    let group = druid::dynamic::DataSourceGroup::new(
        "g3",
        master,
        vec![],
        Arc::new(druid::dynamic::RoundRobinBalancer::new()),
    );
    let ds = DynamicDataSource::new(group);
    let pool = ds.route(SqlHint::Auto);
    assert_eq!(pool.name(), "master");
}

#[test]
fn dynamic_switch() {
    let g1 = druid::dynamic::DataSourceGroup::new(
        "v1",
        MockPool::arc("m1"),
        vec![],
        Arc::new(druid::dynamic::RoundRobinBalancer::new()),
    );
    let ds = DynamicDataSource::new(g1);
    assert_eq!(ds.current_name(), "v1");

    let g2 = druid::dynamic::DataSourceGroup::new(
        "v2",
        MockPool::arc("m2"),
        vec![],
        Arc::new(druid::dynamic::RoundRobinBalancer::new()),
    );
    ds.switch(g2);
    assert_eq!(ds.current_name(), "v2");
}

#[test]
fn dynamic_current_snapshot() {
    let g = druid::dynamic::DataSourceGroup::new(
        "snap",
        MockPool::arc("m"),
        vec![],
        Arc::new(druid::dynamic::RoundRobinBalancer::new()),
    );
    let ds = DynamicDataSource::new(g);
    let current = ds.current();
    assert_eq!(current.name, "snap");
}

#[test]
fn dynamic_route_read_with_slaves() {
    let master = MockPool::arc("master");
    let slave = MockPool::arc("slave");
    let group = druid::dynamic::DataSourceGroup::new(
        "g4",
        master,
        vec![slave],
        Arc::new(druid::dynamic::RoundRobinBalancer::new()),
    );
    let ds = DynamicDataSource::new(group);
    let pool = ds.route(SqlHint::Read);
    assert_eq!(pool.name(), "slave");
}

// ===========================================================================
// LoadBalancer
// ===========================================================================

#[test]
fn round_robin_single_pool() {
    let lb = druid::dynamic::RoundRobinBalancer::new();
    let pools: Vec<Arc<dyn Pool>> = vec![MockPool::arc("only")];
    assert_eq!(lb.pick(&pools).unwrap().name(), "only");
    assert_eq!(lb.pick(&pools).unwrap().name(), "only");
}

#[test]
fn random_balancer_single() {
    let lb = druid::dynamic::RandomBalancer;
    let pools: Vec<Arc<dyn Pool>> = vec![MockPool::arc("only")];
    assert_eq!(lb.pick(&pools).unwrap().name(), "only");
}

#[test]
fn round_robin_default() {
    let lb = druid::dynamic::RoundRobinBalancer::default();
    assert_eq!(lb.name(), "round_robin");
}

// ===========================================================================
// DataSourceGroup / SqlHint
// ===========================================================================

#[test]
fn datasource_group_new() {
    let g = druid::dynamic::DataSourceGroup::new(
        "group1",
        MockPool::arc("master"),
        vec![MockPool::arc("s1"), MockPool::arc("s2")],
        Arc::new(druid::dynamic::RoundRobinBalancer::new()),
    );
    assert_eq!(g.name, "group1");
    assert_eq!(g.slaves.len(), 2);
}

#[test]
fn sql_hint_traits() {
    let hint = SqlHint::Read;
    let cloned = hint.clone();
    assert_eq!(hint, cloned);
    let debug = format!("{hint:?}");
    assert!(debug.contains("Read"));
}

#[test]
fn sql_hint_all_variants() {
    assert_ne!(SqlHint::Read, SqlHint::Write);
    assert_ne!(SqlHint::Read, SqlHint::Auto);
    assert_ne!(SqlHint::Write, SqlHint::Auto);
}
