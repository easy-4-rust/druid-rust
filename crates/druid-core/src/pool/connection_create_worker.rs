//! Rust 异步资源义务：受监管的连接补池 worker。
//!
//! 对应 Java `DruidDataSource.CreateConnectionThread` 与 `emptySignal(fillCount)` 的
//! 产品语义。Rust 的普通获取 future 仍可直接建连，但 recycle/discard 后需要
//! 一个每池唯一、可等待关闭的后台任务把容量补到指定目标。

use super::PoolInner;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

/// 顺序处理补池目标的每池唯一 worker。
///
/// `Some(to_count)` 表示把 active + pooling 补到该目标；`None` 表示关闭。
pub(crate) struct ConnectionCreateWorker {
    inner: Arc<PoolInner>,
    receiver: UnboundedReceiver<Option<usize>>,
}

impl ConnectionCreateWorker {
    /// 创建尚未启动的补池 worker。
    pub(crate) fn new(inner: Arc<PoolInner>, receiver: UnboundedReceiver<Option<usize>>) -> Self {
        Self { inner, receiver }
    }

    /// 启动 worker 并返回由 `DruidPool` 持有的任务句柄。
    pub(crate) fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            'worker: while let Some(command) = self.receiver.recv().await {
                let Some(mut to_count) = command else {
                    break;
                };
                let mut error_count = 0usize;
                loop {
                    match self.inner.fill(to_count).await {
                        Ok(_) => break,
                        Err(error) if self.inner.is_closed() => {
                            tracing::debug!(%error, "stop refill because datasource is closed");
                            break 'worker;
                        }
                        Err(error) => {
                            tracing::warn!(%error, to_count, "refill datasource failed");
                            error_count = error_count.saturating_add(1);
                            if error_count <= self.inner.config.connection_error_retry_attempts {
                                continue;
                            }

                            let delay = self.inner.config.time_between_connect_error;
                            if delay.is_zero() {
                                // Java 只在正退避间隔下发布 failContinuous。
                                tokio::select! {
                                    biased;
                                    command = self.receiver.recv() => {
                                        match command {
                                            Some(Some(next_target)) => {
                                                to_count = to_count.max(next_target);
                                            }
                                            Some(None) | None => break 'worker,
                                        }
                                    }
                                    () = tokio::task::yield_now() => {}
                                }
                                continue;
                            }

                            self.inner.set_fail_continuous(true);
                            if self.inner.config.break_after_acquire_failure {
                                // Java creator 在该配置下终止；pool 生命周期仍持有
                                // 已完成的 JoinHandle，可在 close/restart 中等待。
                                break 'worker;
                            }
                            error_count = 0;

                            let sleep = tokio::time::sleep(delay);
                            tokio::pin!(sleep);
                            loop {
                                tokio::select! {
                                    command = self.receiver.recv() => {
                                        match command {
                                            Some(Some(next_target)) => {
                                                to_count = to_count.max(next_target);
                                            }
                                            Some(None) | None => break 'worker,
                                        }
                                    }
                                    () = &mut sleep => break,
                                }
                            }
                        }
                    }
                }
            }
        })
    }
}
