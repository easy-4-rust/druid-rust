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
    aggregator_handle: Option<JoinHandle<()>>,
    exporter_handle: Option<JoinHandle<()>>,
    unregister_tx: mpsc::UnboundedSender<u64>,
    cancel_token: CancellationToken,
    self_metrics: Arc<RuntimeSelfMetrics>,
}

impl DruidMetricsRuntime {
    /// Start the metrics runtime with the given configuration.
    ///
    /// Spawns sampler, aggregator, and exporter tasks under a supervisor.
    #[allow(clippy::unused_async)]
    pub async fn start(config: DruidMetricsConfig) -> Result<Self, MetricsError> {
        let cancel_token = CancellationToken::new();
        let self_metrics = Arc::new(RuntimeSelfMetrics::new());
        let (snapshot_tx, snapshot_rx) = mpsc::channel(config.queue_capacity);
        let (_agg_cmd_tx, agg_cmd_rx) = mpsc::unbounded_channel();
        let (unregister_tx, unregister_rx) = mpsc::unbounded_channel();

        // Single shared registry: runtime::register() writes, sampler reads + cleans
        let registry: Arc<RwLock<Vec<RegistryEntry>>> = Arc::new(RwLock::new(Vec::new()));

        // Sampler task -- uses the shared registry directly
        let sampler_registry = Arc::clone(&registry);
        let sampler_cancel = cancel_token.clone();
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
        let agg_cancel = cancel_token.clone();
        let agg_self_metrics = Arc::clone(&self_metrics);
        let aggregator_handle = tokio::spawn(async move {
            crate::aggregator::run_aggregator(
                snapshot_rx,
                agg_cmd_rx,
                agg_cancel,
                agg_self_metrics,
            )
            .await;
        });

        // Exporter task (V1: no-op placeholder)
        let exp_cancel = cancel_token.clone();
        let exporter_handle = tokio::spawn(async move {
            exp_cancel.cancelled().await;
        });

        let runtime = Self {
            config,
            registry: Arc::clone(&registry),
            sampler_handle: Some(sampler_handle),
            aggregator_handle: Some(aggregator_handle),
            exporter_handle: Some(exporter_handle),
            unregister_tx,
            cancel_token,
            self_metrics,
        };

        // Unregister listener task: removes entries from registry when guards are dropped
        let mut unreg_rx = unregister_rx;
        let unreg_registry = Arc::clone(&registry);
        tokio::spawn(async move {
            while let Some(id) = unreg_rx.recv().await {
                let mut guard = unreg_registry.write();
                guard.retain(|e| e.datasource_id != id);
            }
        });

        Ok(runtime)
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
        // Step 1: Cancel all tasks
        self.cancel_token.cancel();

        // Step 3-5: Join tasks with deadline
        let join_all = async {
            if let Some(handle) = self.sampler_handle.take() {
                handle.await.ok();
            }
            if let Some(handle) = self.aggregator_handle.take() {
                handle.await.ok();
            }
            if let Some(handle) = self.exporter_handle.take() {
                handle.await.ok();
            }
        };

        if let Ok(()) = tokio::time::timeout(deadline, join_all).await {
            Ok(())
        } else {
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
