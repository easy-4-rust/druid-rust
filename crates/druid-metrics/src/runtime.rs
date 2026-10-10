use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;

use druid::stats::DataSourceMonitorable;
use parking_lot::RwLock;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::config::DruidMetricsConfig;
use crate::error::MetricsError;
use crate::registry::{RegistrationGuard, RegistryEntry};
use crate::self_metrics::RuntimeSelfMetrics;
use crate::supervisor::MetricsSupervisor;
#[cfg(feature = "client")]
use crate::{grpc_client::GrpcClientConfig, grpc_client::GrpcMetricsClient};

/// Non-blocking local metrics runtime.
///
/// Orchestrates sampler, aggregator, and exporter tasks behind a supervisor.
/// All datasource references are held as `Weak` so dropping a datasource
/// automatically stops its metrics collection on the next sample cycle.
pub struct DruidMetricsRuntime {
    config: DruidMetricsConfig,
    /// Shared registry: runtime writes, sampler reads/writes (cleanup).
    registry: Arc<RwLock<Vec<RegistryEntry>>>,
    supervisor: MetricsSupervisor,
    sampler_done: oneshot::Receiver<()>,
    aggregator_done: oneshot::Receiver<()>,
    unregister_tx: mpsc::Sender<u64>,
    aggregator_cancel_token: CancellationToken,
    self_metrics: Arc<RuntimeSelfMetrics>,
}

impl DruidMetricsRuntime {
    /// Start the metrics runtime with the given configuration.
    ///
    /// Spawns sampler, aggregator, and exporter tasks under a supervisor.
    pub async fn start(config: DruidMetricsConfig) -> Result<Self, MetricsError> {
        Self::start_inner(config, RuntimeExporterConfig::Drain).await
    }

    /// 启动指标 runtime，并把聚合输出接入真实 Tonic Admin 客户端。
    #[cfg(feature = "client")]
    pub async fn start_with_grpc(
        config: DruidMetricsConfig,
        grpc_config: GrpcClientConfig,
    ) -> Result<Self, MetricsError> {
        Self::start_inner(config, RuntimeExporterConfig::Grpc(Box::new(grpc_config))).await
    }

    async fn start_inner(
        config: DruidMetricsConfig,
        exporter_config: RuntimeExporterConfig,
    ) -> Result<Self, MetricsError> {
        let aggregator_cancel_token = CancellationToken::new();
        let ownership = Arc::new(crate::pipeline_ownership::PipelineOwnership::new());
        let self_metrics = Arc::new(RuntimeSelfMetrics::with_ownership(Arc::clone(&ownership)));
        let mut supervisor = MetricsSupervisor::new(Arc::clone(&self_metrics));
        let (snapshot_tx, snapshot_rx) = mpsc::channel(config.queue_capacity);
        let (batch_tx, mut batch_rx) = mpsc::channel(config.queue_capacity);
        let (_aggregator_command_tx, aggregator_command_rx) = mpsc::channel(config.queue_capacity);
        let (unregister_tx, unregister_rx) = mpsc::channel(config.queue_capacity);

        // Single shared registry: runtime::register() writes, sampler reads + cleans
        let registry: Arc<RwLock<Vec<RegistryEntry>>> = Arc::new(RwLock::new(Vec::new()));

        let exporter = match exporter_config {
            RuntimeExporterConfig::Drain => RuntimeExporter::Drain,
            #[cfg(feature = "client")]
            RuntimeExporterConfig::Grpc(grpc_config) => {
                let command_executor = crate::registry::RegistryCommandExecutor::new(
                    Arc::clone(&registry),
                    config.batch_size,
                    config.max_sql_statements,
                    config.sql_text_policy,
                );
                let client = GrpcMetricsClient::connect_with_command_executor_and_ownership(
                    *grpc_config,
                    Some(command_executor),
                    ownership,
                )
                .await?;
                RuntimeExporter::Grpc(client)
            }
        };

        // Sampler task -- uses the shared registry directly
        let sampler_registry = Arc::clone(&registry);
        let sampler_cancel = supervisor.cancel_token();
        let sampler_interval = config.sample_interval;
        let sampler_snapshot_tx = snapshot_tx;
        let sampler_self_metrics = Arc::clone(&self_metrics);

        let (sampler_done_tx, sampler_done) = oneshot::channel();
        supervisor.spawn(async move {
            crate::sampler::run_sampler(
                sampler_registry,
                sampler_snapshot_tx,
                sampler_cancel,
                sampler_interval,
                sampler_self_metrics,
            )
            .await;
            let _ = sampler_done_tx.send(());
            Ok(())
        });

        // Aggregator task
        let agg_cancel = aggregator_cancel_token.clone();
        let agg_self_metrics = Arc::clone(&self_metrics);
        let aggregator_config = config.clone();
        let (aggregator_done_tx, aggregator_done) = oneshot::channel();
        supervisor.spawn(async move {
            let result = crate::aggregator::run_aggregator(
                snapshot_rx,
                batch_tx,
                aggregator_command_rx,
                agg_cancel,
                aggregator_config,
                agg_self_metrics,
            )
            .await;
            let _ = aggregator_done_tx.send(());
            result
        });

        #[cfg(feature = "client")]
        let exporter_shutdown_timeout = config.shutdown_timeout;
        let exporter_self_metrics = Arc::clone(&self_metrics);
        supervisor.spawn(async move {
            match exporter {
                RuntimeExporter::Drain => {
                    while batch_rx.recv().await.is_some() {
                        exporter_self_metrics
                            .decrement_pending_export_batches()
                            .map_err(|error| MetricsError::ChannelSend(error.to_string()))?;
                    }
                    Ok(())
                }
                #[cfg(feature = "client")]
                RuntimeExporter::Grpc(client) => {
                    while let Some(batch) = batch_rx.recv().await {
                        client.send_batch_from_exporter(batch).await?;
                    }
                    client.shutdown(exporter_shutdown_timeout).await
                }
            }
        });

        // Unregister listener task: removes entries from registry when guards are dropped
        let mut unreg_rx = unregister_rx;
        let unreg_registry = Arc::clone(&registry);
        let unregister_cancel = supervisor.cancel_token();
        let unregister_metrics = Arc::clone(&self_metrics);
        unregister_metrics.set_unregister_listener_active(true);
        let active_guard = UnregisterListenerActiveGuard(Arc::clone(&unregister_metrics));
        supervisor.spawn(async move {
            let _active_guard = active_guard;
            loop {
                tokio::select! {
                    biased;
                    () = unregister_cancel.cancelled() => break,
                    id = unreg_rx.recv() => {
                        let Some(_id) = id else {
                            break;
                        };
                        let mut guard = unreg_registry.write();
                        // ID 可以被多个注册共享；通知只是唤醒，失效令牌才决定删除。
                        guard.retain(RegistryEntry::is_active);
                        unregister_metrics.set_registered_datasources(guard.len());
                    }
                }
            }
            Ok(())
        });

        Ok(Self {
            config,
            registry: Arc::clone(&registry),
            supervisor,
            sampler_done,
            aggregator_done,
            unregister_tx,
            aggregator_cancel_token,
            self_metrics,
        })
    }

    /// Register a datasource for metrics collection.
    ///
    /// Returns a [`RegistrationGuard`]; when the guard is dropped the datasource
    /// is automatically unregistered. The runtime holds only a `Weak` reference
    /// to the datasource.
    pub fn register(&self, source: Arc<dyn DataSourceMonitorable>) -> RegistrationGuard {
        let identity = source.identity();
        let weak: Weak<dyn DataSourceMonitorable> = Arc::downgrade(&source);

        let entry = RegistryEntry::new(identity.id, weak.clone());
        let active = entry.active_token();

        // Write directly to the shared registry -- the sampler reads from it
        {
            let mut guard = self.registry.write();
            guard.push(entry);
            self.self_metrics.set_registered_datasources(guard.len());
        }

        RegistrationGuard::new(
            identity.id,
            weak,
            self.unregister_tx.clone(),
            active,
            Arc::clone(&self.self_metrics),
        )
    }

    /// Gracefully shut down the runtime.
    ///
    /// 1. Cancel the sampler (stop polling datasources)
    /// 2. Close the producer channel
    /// 3. Flush the aggregator
    /// 4. Wait for exporter to finish
    /// 5. Join all tasks
    ///
    /// Returns `Err(MetricsError::ShutdownTimeout)` with the count of unflushed
    /// snapshots if the deadline is exceeded.
    pub async fn shutdown(mut self, deadline: Duration) -> Result<(), MetricsError> {
        // Stop admission first so the aggregator can drain a stable input queue.
        self.supervisor.cancel();
        let self_metrics = Arc::clone(&self.self_metrics);

        // 采样器先退出并关闭 snapshot sender；聚合器随后刷新并关闭 batch sender；
        // 导出器完成传输后，由同一个 JoinSet 收集全部任务失败。
        let join_all = async {
            let _ = (&mut self.sampler_done).await;
            self.aggregator_cancel_token.cancel();
            let _ = (&mut self.aggregator_done).await;
            let task_result = self.supervisor.join_all().await;
            let unflushed = pipeline_unflushed(&self_metrics);
            if unflushed != 0 && task_result.is_ok() {
                return Err(MetricsError::UnflushedSnapshots { unflushed });
            }
            task_result
        };

        let result = if let Ok(result) = tokio::time::timeout(deadline, join_all).await {
            result
        } else {
            self.aggregator_cancel_token.cancel();
            self.supervisor.abort_all();
            let unflushed = pipeline_unflushed(&self.self_metrics);
            Err(MetricsError::ShutdownTimeout { unflushed })
        };
        // guard 可以比 runtime 活得更久；撤销剩余注册并同步对外计数。
        self.registry.write().clear();
        self_metrics.set_registered_datasources(0);
        result
    }

    /// Returns the runtime configuration.
    pub fn config(&self) -> &DruidMetricsConfig {
        &self.config
    }

    /// Returns a reference to the runtime self-metrics.
    pub fn self_metrics(&self) -> &Arc<RuntimeSelfMetrics> {
        &self.self_metrics
    }
}

impl Drop for DruidMetricsRuntime {
    fn drop(&mut self) {
        // 即使调用方取消 shutdown future，监督器仍持有并终止全部后台任务。
        self.aggregator_cancel_token.cancel();
        self.supervisor.cancel();
        self.supervisor.abort_all();
        self.registry.write().clear();
        self.self_metrics.set_registered_datasources(0);
    }
}

enum RuntimeExporterConfig {
    Drain,
    #[cfg(feature = "client")]
    Grpc(Box<GrpcClientConfig>),
}

enum RuntimeExporter {
    Drain,
    #[cfg(feature = "client")]
    Grpc(GrpcMetricsClient),
}

struct UnregisterListenerActiveGuard(Arc<RuntimeSelfMetrics>);

impl Drop for UnregisterListenerActiveGuard {
    fn drop(&mut self) {
        self.0.set_unregister_listener_active(false);
    }
}

fn pipeline_unflushed(self_metrics: &RuntimeSelfMetrics) -> usize {
    self_metrics.total_unflushed()
}
