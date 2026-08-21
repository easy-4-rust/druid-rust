use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;

use druid::stats::DataSourceMonitorable;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::config::DruidMetricsConfig;
use crate::error::MetricsError;
use crate::registry::{RegistrationGuard, RegistryEntry};
use crate::self_metrics::RuntimeSelfMetrics;

/// Non-blocking local metrics runtime.
///
/// Orchestrates sampler, aggregator, and exporter tasks behind a supervisor.
/// All datasource references are held as `Weak` so dropping a datasource
/// automatically stops its metrics collection on the next sample cycle.
pub struct DruidMetricsRuntime {
    config: DruidMetricsConfig,
    /// Shared registry: runtime writes, sampler reads/writes (cleanup).
    registry: Arc<RwLock<Vec<RegistryEntry>>>,
    sampler_handle: Option<JoinHandle<()>>,
    aggregator_handle: Option<JoinHandle<Result<(), MetricsError>>>,
    exporter_handle: Option<JoinHandle<()>>,
    unregister_handle: Option<JoinHandle<()>>,
    unregister_tx: mpsc::UnboundedSender<u64>,
    sampler_cancel_token: CancellationToken,
    aggregator_cancel_token: CancellationToken,
    unregister_cancel_token: CancellationToken,
    self_metrics: Arc<RuntimeSelfMetrics>,
}

impl DruidMetricsRuntime {
    /// Start the metrics runtime with the given configuration.
    ///
    /// Spawns sampler, aggregator, and exporter tasks under a supervisor.
    #[allow(clippy::unused_async)]
    pub async fn start(config: DruidMetricsConfig) -> Result<Self, MetricsError> {
        let sampler_cancel_token = CancellationToken::new();
        let aggregator_cancel_token = CancellationToken::new();
        let unregister_cancel_token = CancellationToken::new();
        let self_metrics = Arc::new(RuntimeSelfMetrics::new());
        let (snapshot_tx, snapshot_rx) = mpsc::channel(config.queue_capacity);
        let (batch_tx, mut batch_rx) = mpsc::channel(config.queue_capacity);
        let (_aggregator_command_tx, aggregator_command_rx) = mpsc::channel(config.queue_capacity);
        let (unregister_tx, unregister_rx) = mpsc::unbounded_channel();

        // Single shared registry: runtime::register() writes, sampler reads + cleans
        let registry: Arc<RwLock<Vec<RegistryEntry>>> = Arc::new(RwLock::new(Vec::new()));

        // Sampler task -- uses the shared registry directly
        let sampler_registry = Arc::clone(&registry);
        let sampler_cancel = sampler_cancel_token.clone();
        let sampler_interval = config.sample_interval;
        let sampler_snapshot_tx = snapshot_tx;
        let sampler_self_metrics = Arc::clone(&self_metrics);

        let sampler_handle = tokio::spawn(async move {
            crate::sampler::run_sampler(
                sampler_registry,
                sampler_snapshot_tx,
                sampler_cancel,
                sampler_interval,
                sampler_self_metrics,
            )
            .await;
        });

        // Aggregator task
        let agg_cancel = aggregator_cancel_token.clone();
        let agg_self_metrics = Arc::clone(&self_metrics);
        let aggregator_config = config.clone();
        let aggregator_handle = tokio::spawn(async move {
            crate::aggregator::run_aggregator(
                snapshot_rx,
                batch_tx,
                aggregator_command_rx,
                agg_cancel,
                aggregator_config,
                agg_self_metrics,
            )
            .await
        });

        // Exporter task (V1: drain-only placeholder; Task 4 adds network transport).
        let exporter_handle = tokio::spawn(async move { while batch_rx.recv().await.is_some() {} });

        // Unregister listener task: removes entries from registry when guards are dropped
        let mut unreg_rx = unregister_rx;
        let unreg_registry = Arc::clone(&registry);
        let unregister_cancel = unregister_cancel_token.clone();
        let unregister_metrics = Arc::clone(&self_metrics);
        unregister_metrics.set_unregister_listener_active(true);
        let unregister_handle = tokio::spawn(async move {
            let _active_guard = UnregisterListenerActiveGuard(unregister_metrics);
            loop {
                tokio::select! {
                    biased;
                    () = unregister_cancel.cancelled() => break,
                    id = unreg_rx.recv() => {
                        let Some(id) = id else {
                            break;
                        };
                        let mut guard = unreg_registry.write();
                        guard.retain(|entry| entry.datasource_id != id);
                    }
                }
            }
        });

        Ok(Self {
            config,
            registry: Arc::clone(&registry),
            sampler_handle: Some(sampler_handle),
            aggregator_handle: Some(aggregator_handle),
            exporter_handle: Some(exporter_handle),
            unregister_handle: Some(unregister_handle),
            unregister_tx,
            sampler_cancel_token,
            aggregator_cancel_token,
            unregister_cancel_token,
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

        let entry = RegistryEntry {
            datasource_id: identity.id,
            weak_ref: weak.clone(),
        };

        // Write directly to the shared registry -- the sampler reads from it
        {
            let mut guard = self.registry.write();
            guard.push(entry);
        }

        RegistrationGuard::new(identity.id, weak, self.unregister_tx.clone())
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
        self.sampler_cancel_token.cancel();
        self.unregister_cancel_token.cancel();

        let aggregator_cancel = self.aggregator_cancel_token.clone();
        let mut sampler_handle = self.sampler_handle.take();
        let mut aggregator_handle = self.aggregator_handle.take();
        let mut exporter_handle = self.exporter_handle.take();
        let mut unregister_handle = self.unregister_handle.take();
        let self_metrics = Arc::clone(&self.self_metrics);

        // Join tasks with one shared deadline. The drain-only exporter exits after
        // the aggregator flushes and closes the bounded batch channel.
        let join_all = async {
            let mut first_error = None;
            if let Some(handle) = sampler_handle.as_mut() {
                if let Err(error) = handle.await {
                    first_error = Some(MetricsError::Join(error));
                }
            }
            aggregator_cancel.cancel();
            if let Some(handle) = aggregator_handle.as_mut() {
                match handle.await {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                    Err(error) => {
                        if first_error.is_none() {
                            first_error = Some(MetricsError::Join(error));
                        }
                    }
                }
            }
            if let Some(handle) = exporter_handle.as_mut() {
                if let Err(error) = handle.await {
                    if first_error.is_none() {
                        first_error = Some(MetricsError::Join(error));
                    }
                }
            }
            if let Some(handle) = unregister_handle.as_mut() {
                if let Err(error) = handle.await {
                    if first_error.is_none() {
                        first_error = Some(MetricsError::Join(error));
                    }
                }
            }

            let unflushed = self_metrics.pending_snapshots();
            if unflushed != 0 && first_error.is_none() {
                first_error = Some(MetricsError::UnflushedSnapshots { unflushed });
            }

            first_error.map_or(Ok(()), Err)
        };

        if let Ok(result) = tokio::time::timeout(deadline, join_all).await {
            result
        } else {
            self.aggregator_cancel_token.cancel();
            self.unregister_cancel_token.cancel();
            abort_if_running(sampler_handle.as_mut()).await;
            abort_if_running(aggregator_handle.as_mut()).await;
            abort_if_running(exporter_handle.as_mut()).await;
            abort_if_running(unregister_handle.as_mut()).await;
            let unflushed = self.self_metrics.pending_snapshots();
            Err(MetricsError::ShutdownTimeout { unflushed })
        }
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

struct UnregisterListenerActiveGuard(Arc<RuntimeSelfMetrics>);

impl Drop for UnregisterListenerActiveGuard {
    fn drop(&mut self) {
        self.0.set_unregister_listener_active(false);
    }
}

async fn abort_if_running<T>(handle: Option<&mut JoinHandle<T>>) {
    if let Some(handle) = handle {
        if !handle.is_finished() {
            handle.abort();
            let _ = handle.await;
        }
    }
}
