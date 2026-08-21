use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use druid::stats::DataSourceMonitorable;
use parking_lot::RwLock;
use thiserror::Error;

use crate::aggregator::AggregatedBatch;
use crate::config::SqlTextPolicy;
use crate::self_metrics::RuntimeSelfMetrics;
use crate::snapshot_encoder::encode_snapshot;

/// Guard that keeps a datasource registered in the metrics runtime.
///
/// When dropped, the datasource is automatically unregistered.
#[derive(Debug)]
pub struct RegistrationGuard {
    datasource_id: u64,
    _weak_ref: Weak<dyn DataSourceMonitorable>,
    unregister_tx: Option<tokio::sync::mpsc::Sender<u64>>,
    active: Arc<AtomicBool>,
    self_metrics: Arc<RuntimeSelfMetrics>,
}

impl RegistrationGuard {
    pub fn new(
        datasource_id: u64,
        weak_ref: Weak<dyn DataSourceMonitorable>,
        unregister_tx: tokio::sync::mpsc::Sender<u64>,
        active: Arc<AtomicBool>,
        self_metrics: Arc<RuntimeSelfMetrics>,
    ) -> Self {
        Self {
            datasource_id,
            _weak_ref: weak_ref,
            unregister_tx: Some(unregister_tx),
            active,
            self_metrics,
        }
    }

    /// Returns the datasource ID associated with this guard.
    pub fn datasource_id(&self) -> u64 {
        self.datasource_id
    }
}

impl Drop for RegistrationGuard {
    fn drop(&mut self) {
        if let Some(tx) = self.unregister_tx.take() {
            self.active.store(false, Ordering::Release);
            if matches!(
                tx.try_send(self.datasource_id),
                Err(tokio::sync::mpsc::error::TrySendError::Full(_))
            ) {
                // 队列内已有 wake；listener 收到任一 ID 都会合并清理全部 inactive entry。
                self.self_metrics.increment_unregister_coalesced_total();
            }
        }
    }
}

/// Internal registry entry holding a weak reference to a datasource.
#[derive(Debug, Clone)]
pub struct RegistryEntry {
    pub datasource_id: u64,
    pub weak_ref: Weak<dyn DataSourceMonitorable>,
    active: Arc<AtomicBool>,
}

impl RegistryEntry {
    /// 创建一个 active registry entry。
    #[must_use]
    pub fn new(datasource_id: u64, weak_ref: Weak<dyn DataSourceMonitorable>) -> Self {
        Self {
            datasource_id,
            weak_ref,
            active: Arc::new(AtomicBool::new(true)),
        }
    }

    /// 返回 registration guard 与 sampler 共用的 active token。
    pub(crate) fn active_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.active)
    }

    /// 返回 registration 是否仍接受采样/命令。
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
}

/// 远程管理命令访问 datasource registry 时产生的错误。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryCommandError {
    /// ResetStats 没有给出任何目标 datasource。
    #[error("reset command has no target datasource IDs")]
    EmptyResetTargets,

    /// 请求的 datasource ID 未注册。
    #[error("datasource IDs are not registered: {0:?}")]
    UnknownDatasources(Vec<u64>),

    /// datasource 已释放或当前无法生成快照。
    #[error("datasource {datasource_id} is unavailable: {reason}")]
    DatasourceUnavailable {
        /// 不可用 datasource ID。
        datasource_id: u64,
        /// 不可用原因。
        reason: String,
    },

    /// registry 中没有可用于全量快照的数据源。
    #[error("no registered datasource is available for a full snapshot")]
    EmptyRegistry,

    /// 全量快照批次上限无效。
    #[error("full snapshot batch size must be greater than zero")]
    InvalidBatchSize,
}

/// 为 PushWorker 提供真实 datasource reset 与全量快照的 registry 命令边界。
///
/// 执行器只持有共享 registry；每次命令执行前先完成目标解析，避免未知 ID 导致部分重置。
#[derive(Clone)]
pub struct RegistryCommandExecutor {
    registry: Arc<RwLock<Vec<RegistryEntry>>>,
    batch_size: usize,
    max_sql_statements: usize,
    sql_text_policy: SqlTextPolicy,
}

impl RegistryCommandExecutor {
    /// 使用共享 registry 和现有隐私/数量配置创建命令执行器。
    #[must_use]
    pub fn new(
        registry: Arc<RwLock<Vec<RegistryEntry>>>,
        batch_size: usize,
        max_sql_statements: usize,
        sql_text_policy: SqlTextPolicy,
    ) -> Self {
        Self {
            registry,
            batch_size,
            max_sql_statements,
            sql_text_policy,
        }
    }

    /// 按 ID 重置已注册 datasource；重复 ID 只执行一次。
    ///
    /// 所有目标都会在第一次副作用前完成注册与存活校验。只有全部目标重置完成才返回成功。
    pub fn reset_stats(&self, target_datasource_ids: &[u64]) -> Result<(), RegistryCommandError> {
        let target_ids = target_datasource_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if target_ids.is_empty() {
            return Err(RegistryCommandError::EmptyResetTargets);
        }

        let registry = self.registry.read().clone();
        let unknown = target_ids
            .iter()
            .filter(|datasource_id| {
                !registry
                    .iter()
                    .any(|entry| entry.datasource_id == **datasource_id && entry.is_active())
            })
            .copied()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            return Err(RegistryCommandError::UnknownDatasources(unknown));
        }

        let mut registered = BTreeMap::new();
        for datasource_id in &target_ids {
            let entry = registry
                .iter()
                .find(|entry| entry.datasource_id == *datasource_id && entry.is_active())
                .expect("target registration was validated");
            let source = entry.weak_ref.upgrade().ok_or_else(|| {
                RegistryCommandError::DatasourceUnavailable {
                    datasource_id: *datasource_id,
                    reason: "registered datasource was dropped".to_owned(),
                }
            })?;
            registered.insert(*datasource_id, source);
        }

        for datasource_id in target_ids {
            registered
                .get(&datasource_id)
                .expect("targets were validated before reset")
                .reset_stat();
        }
        Ok(())
    }

    /// 从当前全部已注册 datasource 生成真实且受 batch_size 限制的全量批次。
    ///
    /// 空 registry 仍返回一个空权威批次，用于原子清空远端 stream。快照沿用
    /// Task 2 的数量限制和 SQL 隐私编码，不包含原始 SQL 或 bind 值。
    pub fn full_snapshot(&self) -> Result<Vec<AggregatedBatch>, RegistryCommandError> {
        if self.batch_size == 0 {
            return Err(RegistryCommandError::InvalidBatchSize);
        }
        let registered = self.live_datasources_by_id()?;
        let mut entries = VecDeque::with_capacity(registered.len());
        for (datasource_id, source) in registered {
            let snapshot = source.try_snapshot().map_err(|error| {
                RegistryCommandError::DatasourceUnavailable {
                    datasource_id,
                    reason: format!("{error:?}"),
                }
            })?;
            entries.push_back(encode_snapshot(
                snapshot,
                self.max_sql_statements,
                self.sql_text_policy,
            ));
        }

        let mut batches = Vec::new();
        if entries.is_empty() {
            batches.push(AggregatedBatch {
                entries: Vec::new(),
                is_full_snapshot: true,
            });
            return Ok(batches);
        }
        while !entries.is_empty() {
            let take = self.batch_size.min(entries.len());
            batches.push(AggregatedBatch {
                entries: entries.drain(..take).collect(),
                is_full_snapshot: true,
            });
        }
        Ok(batches)
    }

    fn live_datasources_by_id(
        &self,
    ) -> Result<BTreeMap<u64, Arc<dyn DataSourceMonitorable>>, RegistryCommandError> {
        let registry = self.registry.read().clone();
        let mut live = BTreeMap::new();
        for entry in registry {
            if !entry.is_active() {
                continue;
            }
            if live.contains_key(&entry.datasource_id) {
                continue;
            }
            let source = entry.weak_ref.upgrade().ok_or_else(|| {
                RegistryCommandError::DatasourceUnavailable {
                    datasource_id: entry.datasource_id,
                    reason: "registered datasource was dropped".to_owned(),
                }
            })?;
            live.insert(entry.datasource_id, source);
        }
        Ok(live)
    }
}
