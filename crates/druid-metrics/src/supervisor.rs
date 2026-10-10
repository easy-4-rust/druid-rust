//! Metrics runtime 后台任务的唯一所有者。

use std::future::Future;
use std::sync::Arc;

use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::error::MetricsError;
use crate::self_metrics::RuntimeSelfMetrics;

/// 持有全部后台任务，并统一收集任务错误与关闭时的取消信号。
pub(crate) struct MetricsSupervisor {
    tasks: JoinSet<Result<(), MetricsError>>,
    cancel: CancellationToken,
    self_metrics: Arc<RuntimeSelfMetrics>,
}

impl MetricsSupervisor {
    /// 创建尚未启动任务的监督器。
    pub(crate) fn new(self_metrics: Arc<RuntimeSelfMetrics>) -> Self {
        Self {
            tasks: JoinSet::new(),
            cancel: CancellationToken::new(),
            self_metrics,
        }
    }

    /// 返回停止采样与注册入口的取消令牌。
    pub(crate) fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// 启动一个由监督器持有的后台任务。
    pub(crate) fn spawn<F>(&mut self, task: F)
    where
        F: Future<Output = Result<(), MetricsError>> + Send + 'static,
    {
        self.self_metrics.task_started();
        let guard = ActiveTaskGuard(Arc::clone(&self.self_metrics));
        self.tasks.spawn(async move {
            let _guard = guard;
            task.await
        });
    }

    /// 停止新的采样与注册清理轮次。
    pub(crate) fn cancel(&self) {
        self.cancel.cancel();
    }

    /// 收集所有任务结果，返回首个任务错误。
    pub(crate) async fn join_all(&mut self) -> Result<(), MetricsError> {
        let mut first_error = None;
        while let Some(result) = self.tasks.join_next().await {
            let error = match result {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(error),
                Err(error) => Some(MetricsError::Join(error)),
            };
            if first_error.is_none() {
                first_error = error;
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// 到期时终止仍在运行的任务。
    pub(crate) fn abort_all(&mut self) {
        self.tasks.abort_all();
    }
}

struct ActiveTaskGuard(Arc<RuntimeSelfMetrics>);

impl Drop for ActiveTaskGuard {
    fn drop(&mut self) {
        self.0.task_stopped();
    }
}

impl Drop for MetricsSupervisor {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.tasks.abort_all();
    }
}
