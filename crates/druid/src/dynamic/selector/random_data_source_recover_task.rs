//! 对应 Java 类：`com.alibaba.druid.pool.ha.selector.RandomDataSourceRecoverThread`。

use super::random_data_source_selector::RandomDataSourceSelectorState;
use crate::core::PhysicalConnection;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// 周期探测 selector 黑名单节点并在恢复后摘除黑名单。
pub struct RandomDataSourceRecoverTask {
    state: Arc<RandomDataSourceSelectorState>,
    shutdown: tokio::sync::watch::Receiver<bool>,
}

impl RandomDataSourceRecoverTask {
    pub const DEFAULT_RECOVER_INTERVAL_SECONDS: i32 = 120;

    pub(crate) fn new(
        state: Arc<RandomDataSourceSelectorState>,
        shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Self {
        Self { state, shutdown }
    }

    /// 运行受监管恢复循环，直到收到 shutdown。
    pub async fn run(mut self) {
        loop {
            if *self.shutdown.borrow() {
                return;
            }
            for data_source in self.state.blacklist.read().clone() {
                let result = async {
                    let mut connection = data_source.get().await?;
                    let validation_sleep =
                        self.state.validation_sleep_seconds.load(Ordering::Acquire);
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
                    self.state.remove_blacklist(&data_source);
                }
            }
            let interval = self.state.recovery_interval_seconds.load(Ordering::Acquire);
            if interval < 0 {
                return;
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(
                    u64::try_from(interval).unwrap_or(0),
                )) => {}
                changed = self.shutdown.changed() => {
                    if changed.is_err() || *self.shutdown.borrow() {
                        return;
                    }
                }
            }
        }
    }
}
