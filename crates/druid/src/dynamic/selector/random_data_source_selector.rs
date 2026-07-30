//! 对应 Java 类：`com.alibaba.druid.pool.ha.selector.RandomDataSourceSelector`。

use super::{DataSourceSelector, RandomDataSourceRecoverTask, RandomDataSourceValidateTask};
use crate::core::Pool;
use crate::dynamic::high_available_data_source::HighAvailableDataSourceInner;
use crate::dynamic::HighAvailableDataSource;
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

pub(crate) struct RandomDataSourceSelectorState {
    pub(crate) high_available_data_source: Weak<HighAvailableDataSourceInner>,
    pub(crate) blacklist: RwLock<Vec<Arc<dyn Pool>>>,
    pub(crate) checking_interval_seconds: AtomicI32,
    pub(crate) recovery_interval_seconds: AtomicI32,
    pub(crate) validation_sleep_seconds: AtomicI32,
    pub(crate) blacklist_threshold: AtomicI32,
    pub(crate) error_counts: DashMap<String, i32>,
    pub(crate) last_check_times: DashMap<String, u64>,
}

impl RandomDataSourceSelectorState {
    pub(crate) fn data_source_map(&self) -> HashMap<String, Arc<dyn Pool>> {
        self.high_available_data_source
            .upgrade()
            .map_or_else(HashMap::new, |data_source| {
                data_source.available_data_source_map()
            })
    }

    pub(crate) fn full_data_source_map(&self) -> HashMap<String, Arc<dyn Pool>> {
        self.high_available_data_source
            .upgrade()
            .map_or_else(HashMap::new, |data_source| data_source.data_source_map())
    }

    pub(crate) fn contains_in_blacklist(&self, data_source: &Arc<dyn Pool>) -> bool {
        self.blacklist
            .read()
            .iter()
            .any(|candidate| Arc::ptr_eq(candidate, data_source))
    }

    pub(crate) fn add_blacklist(&self, data_source: Arc<dyn Pool>) {
        let mut blacklist = self.blacklist.write();
        if !blacklist
            .iter()
            .any(|candidate| Arc::ptr_eq(candidate, &data_source))
        {
            blacklist.push(data_source);
        }
    }

    pub(crate) fn remove_blacklist(&self, data_source: &Arc<dyn Pool>) {
        self.blacklist
            .write()
            .retain(|candidate| !Arc::ptr_eq(candidate, data_source));
    }
}

struct MaintenanceTasks {
    shutdown: Option<tokio::sync::watch::Sender<bool>>,
    handles: Vec<tokio::task::JoinHandle<()>>,
}

impl MaintenanceTasks {
    fn new() -> Self {
        Self {
            shutdown: None,
            handles: Vec::new(),
        }
    }

    fn stop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(true);
        }
        for handle in self.handles.drain(..) {
            handle.abort();
        }
    }
}

/// 随机选择可用且非繁忙数据源，并维护独立验证黑名单。
pub struct RandomDataSourceSelector {
    state: Arc<RandomDataSourceSelectorState>,
    maintenance: Mutex<MaintenanceTasks>,
}

impl RandomDataSourceSelector {
    pub const PROP_CHECKING_INTERVAL: &'static str = "druid.ha.random.checkingIntervalSeconds";
    pub const PROP_RECOVERY_INTERVAL: &'static str = "druid.ha.random.recoveryIntervalSeconds";
    pub const PROP_VALIDATION_SLEEP: &'static str = "druid.ha.random.validationSleepSeconds";
    pub const PROP_BLACKLIST_THRESHOLD: &'static str = "druid.ha.random.blacklistThreshold";

    /// 创建绑定到 HA 数据源的随机选择器。
    #[must_use]
    pub fn new(data_source: &HighAvailableDataSource) -> Self {
        Self {
            state: Arc::new(RandomDataSourceSelectorState {
                high_available_data_source: data_source.weak_inner(),
                blacklist: RwLock::new(Vec::new()),
                checking_interval_seconds: AtomicI32::new(
                    RandomDataSourceValidateTask::DEFAULT_CHECKING_INTERVAL_SECONDS,
                ),
                recovery_interval_seconds: AtomicI32::new(
                    RandomDataSourceRecoverTask::DEFAULT_RECOVER_INTERVAL_SECONDS,
                ),
                validation_sleep_seconds: AtomicI32::new(0),
                blacklist_threshold: AtomicI32::new(
                    RandomDataSourceValidateTask::DEFAULT_BLACKLIST_THRESHOLD,
                ),
                error_counts: DashMap::new(),
                last_check_times: DashMap::new(),
            }),
            maintenance: Mutex::new(MaintenanceTasks::new()),
        }
    }

    /// 返回包含 HA 外层 blacklist 的可用节点。
    #[must_use]
    pub fn data_source_map(&self) -> HashMap<String, Arc<dyn Pool>> {
        self.state.data_source_map()
    }

    /// 返回未应用 HA 外层 blacklist 的全部节点。
    #[must_use]
    pub fn full_data_source_map(&self) -> HashMap<String, Arc<dyn Pool>> {
        self.state.full_data_source_map()
    }

    /// 返回验证黑名单快照。
    #[must_use]
    pub fn blacklist(&self) -> Vec<Arc<dyn Pool>> {
        self.state.blacklist.read().clone()
    }

    /// 判断节点是否在验证黑名单。
    #[must_use]
    pub fn contains_in_blacklist(&self, data_source: &Arc<dyn Pool>) -> bool {
        self.state.contains_in_blacklist(data_source)
    }

    /// 加入验证黑名单；相同数据源只保存一次。
    pub fn add_blacklist(&self, data_source: Arc<dyn Pool>) {
        self.state.add_blacklist(data_source);
    }

    /// 从验证黑名单移除数据源。
    pub fn remove_blacklist(&self, data_source: &Arc<dyn Pool>) {
        self.state.remove_blacklist(data_source);
    }

    fn candidates(&self) -> Vec<Arc<dyn Pool>> {
        let map = self.data_source_map();
        if map.is_empty() {
            return Vec::new();
        }
        let blacklist = self.state.blacklist.read();
        let mut candidates = if blacklist.is_empty() || blacklist.len() >= map.len() {
            map.into_values().collect::<Vec<_>>()
        } else {
            map.into_values()
                .filter(|pool| {
                    !blacklist
                        .iter()
                        .any(|blacklisted| Arc::ptr_eq(blacklisted, pool))
                })
                .collect()
        };
        let busy = candidates
            .iter()
            .filter(|pool| pool.state().idle_count == 0)
            .cloned()
            .collect::<Vec<_>>();
        if !busy.is_empty() && busy.len() < candidates.len() {
            candidates.retain(|pool| !busy.iter().any(|busy| Arc::ptr_eq(busy, pool)));
        }
        candidates
    }

    /// 返回检查间隔秒数。
    #[must_use]
    pub fn checking_interval_seconds(&self) -> i32 {
        self.state.checking_interval_seconds.load(Ordering::Acquire)
    }

    /// 设置检查间隔秒数。
    pub fn set_checking_interval_seconds(&self, value: i32) {
        self.state
            .checking_interval_seconds
            .store(value, Ordering::Release);
    }

    /// 返回恢复间隔秒数。
    #[must_use]
    pub fn recovery_interval_seconds(&self) -> i32 {
        self.state.recovery_interval_seconds.load(Ordering::Acquire)
    }

    /// 设置恢复间隔秒数。
    pub fn set_recovery_interval_seconds(&self, value: i32) {
        self.state
            .recovery_interval_seconds
            .store(value, Ordering::Release);
    }

    /// 返回验证间隔内休眠秒数。
    #[must_use]
    pub fn validation_sleep_seconds(&self) -> i32 {
        self.state.validation_sleep_seconds.load(Ordering::Acquire)
    }

    /// 设置验证间隔内休眠秒数。
    pub fn set_validation_sleep_seconds(&self, value: i32) {
        self.state
            .validation_sleep_seconds
            .store(value, Ordering::Release);
    }

    /// 返回进入黑名单的失败阈值。
    #[must_use]
    pub fn blacklist_threshold(&self) -> i32 {
        self.state.blacklist_threshold.load(Ordering::Acquire)
    }

    /// 设置进入黑名单的失败阈值。
    pub fn set_blacklist_threshold(&self, value: i32) {
        self.state
            .blacklist_threshold
            .store(value, Ordering::Release);
    }
}

impl DataSourceSelector for RandomDataSourceSelector {
    fn get(&self) -> Option<Arc<dyn Pool>> {
        let candidates = self.candidates();
        (!candidates.is_empty()).then(|| {
            let index = fastrand::usize(0..candidates.len());
            Arc::clone(&candidates[index])
        })
    }

    fn set_target(&self, _name: Option<String>) {}

    fn name(&self) -> &'static str {
        "random"
    }

    fn init(&self) {
        let Some(data_source) = self.state.high_available_data_source.upgrade() else {
            return;
        };
        if data_source.test_on_borrow.load(Ordering::Acquire)
            || data_source.test_on_return.load(Ordering::Acquire)
        {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let mut maintenance = self.maintenance.lock();
        maintenance.stop();
        let (shutdown, receiver) = tokio::sync::watch::channel(false);
        let validate = RandomDataSourceValidateTask::new(Arc::clone(&self.state), receiver.clone());
        let recover = RandomDataSourceRecoverTask::new(Arc::clone(&self.state), receiver);
        maintenance.shutdown = Some(shutdown);
        maintenance.handles = vec![runtime.spawn(validate.run()), runtime.spawn(recover.run())];
    }

    fn destroy(&self) {
        self.maintenance.lock().stop();
    }
}
