//! 对应 Java 类：`com.alibaba.druid.pool.ha.selector.RandomDataSourceValidateThread`。

use super::random_data_source_selector::RandomDataSourceSelectorState;
use crate::core::{PhysicalConnection, Pool};
use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

fn success_times() -> &'static DashMap<String, u64> {
    static SUCCESS_TIMES: OnceLock<DashMap<String, u64>> = OnceLock::new();
    SUCCESS_TIMES.get_or_init(DashMap::new)
}

/// 周期验证非黑名单节点并按连续失败次数维护 selector 黑名单。
pub struct RandomDataSourceValidateTask {
    state: Arc<RandomDataSourceSelectorState>,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl RandomDataSourceValidateTask {
    pub const DEFAULT_CHECKING_INTERVAL_SECONDS: i32 = 10;
    pub const DEFAULT_BLACKLIST_THRESHOLD: i32 = 3;

    pub(crate) fn new(
        state: Arc<RandomDataSourceSelectorState>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        Self { state, shutdown }
    }

    /// 记录指定数据源最近一次成功执行时间。
    pub fn log_success_time(data_source_name: &str) {
        if !data_source_name.is_empty() {
            success_times().insert(data_source_name.to_owned(), crate::dynamic::epoch_millis());
        }
    }

    /// 返回指定数据源最近一次成功执行时间。
    #[must_use]
    pub fn success_time(data_source_name: &str) -> Option<u64> {
        success_times().get(data_source_name).map(|time| *time)
    }

    /// 运行受监管验证循环，直到收到 shutdown。
    pub async fn run(mut self) {
        loop {
            if *self.shutdown.borrow() {
                return;
            }
            self.check_all_data_sources().await;
            self.maintain_blacklist();
            self.cleanup();
            let sleep = self.sleep_duration();
            tokio::select! {
                _ = tokio::time::sleep(sleep) => {}
                changed = self.shutdown.changed() => {
                    if changed.is_err() || *self.shutdown.borrow() {
                        return;
                    }
                }
            }
        }
    }

    async fn check_all_data_sources(&self) {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(5));
        let mut tasks = tokio::task::JoinSet::new();
        for data_source in self.state.full_data_source_map().into_values() {
            if self.state.contains_in_blacklist(&data_source) {
                continue;
            }
            let state = Arc::clone(&self.state);
            let semaphore = Arc::clone(&semaphore);
            tasks.spawn(async move {
                let Ok(_permit) = semaphore.acquire_owned().await else {
                    return;
                };
                Self::check_one(state.as_ref(), data_source).await;
            });
        }
        while let Some(result) = tasks.join_next().await {
            if let Err(error) = result {
                tracing::warn!(%error, "HA datasource validation task failed");
            }
        }
    }

    async fn check_one(state: &RandomDataSourceSelectorState, data_source: Arc<dyn Pool>) {
        let name = data_source.name().to_owned();
        if Self::is_skip_checking(state, &name) {
            return;
        }
        let result = async {
            let mut connection = data_source.get().await?;
            let validation_sleep = state.validation_sleep_seconds.load(Ordering::Acquire);
            if validation_sleep > 0 {
                tokio::time::sleep(Duration::from_secs(
                    u64::try_from(validation_sleep).unwrap_or(0),
                ))
                .await;
            }
            PhysicalConnection::ping(&mut connection).await
        }
        .await;
        if result.is_ok() {
            Self::log_success_time(&name);
            state.error_counts.insert(name.clone(), 0);
        } else {
            state
                .error_counts
                .entry(name.clone())
                .and_modify(|count| *count = count.wrapping_add(1))
                .or_insert(1);
        }
        state
            .last_check_times
            .insert(name, crate::dynamic::epoch_millis());
    }

    fn is_skip_checking(state: &RandomDataSourceSelectorState, name: &str) -> bool {
        let Some(last_success_time) = Self::success_time(name) else {
            return false;
        };
        let Some(last_check_time) = state.last_check_times.get(name).map(|time| *time) else {
            return false;
        };
        let now = crate::dynamic::epoch_millis();
        let checking_interval = i64::from(state.checking_interval_seconds.load(Ordering::Acquire))
            .saturating_mul(1_000);
        let error_count = state.error_counts.get(name).map_or(0, |count| *count);
        i128::from(now) - i128::from(last_success_time) <= i128::from(checking_interval)
            && i128::from(now) - i128::from(last_check_time)
                <= i128::from(checking_interval).saturating_mul(5)
            && error_count < 1
    }

    fn maintain_blacklist(&self) {
        let data_sources = self.state.full_data_source_map();
        let threshold = self.state.blacklist_threshold.load(Ordering::Acquire);
        for entry in &self.state.error_counts {
            let name = entry.key();
            let count = *entry.value();
            let data_source = data_sources.get(name).cloned().or_else(|| {
                data_sources
                    .values()
                    .find(|data_source| data_source.name() == name)
                    .cloned()
            });
            let Some(data_source) = data_source else {
                continue;
            };
            if count <= 0 {
                self.state.remove_blacklist(&data_source);
            } else if count >= threshold && !self.state.contains_in_blacklist(&data_source) {
                self.state.add_blacklist(data_source);
            }
        }
    }

    fn cleanup(&self) {
        let names = self
            .state
            .full_data_source_map()
            .into_values()
            .map(|data_source| data_source.name().to_owned())
            .collect::<HashSet<_>>();
        success_times().retain(|name, _| names.contains(name));
        self.state
            .error_counts
            .retain(|name, _| names.contains(name));
        self.state
            .last_check_times
            .retain(|name, _| names.contains(name));
    }

    fn sleep_duration(&self) -> Duration {
        let threshold = self.state.blacklist_threshold.load(Ordering::Acquire);
        let below_threshold = self
            .state
            .error_counts
            .iter()
            .map(|entry| *entry.value())
            .filter(|count| *count > 0 && *count < threshold)
            .max()
            .unwrap_or(0);
        let checking = self.state.checking_interval_seconds.load(Ordering::Acquire);
        let seconds = checking
            .checked_div(below_threshold.saturating_add(1))
            .unwrap_or(0);
        Duration::from_secs(u64::try_from(seconds.max(1)).unwrap_or(1))
    }
}
