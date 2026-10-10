//! Supervised async close worker for canonical holder resources.

use crate::core::{ConnectionFactory, DruidConnectionHolder, FilterChain};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use tokio::sync::OwnedSemaphorePermit;

pub(crate) struct ConnectionCloseCommand {
    pub(crate) connection_id: u64,
    pub(crate) connection_age: Duration,
    pub(crate) holder: Box<DruidConnectionHolder>,
    pub(crate) queue_permit: OwnedSemaphorePermit,
}

pub(crate) struct ConnectionCloseWorker {
    _factory: Arc<dyn ConnectionFactory>,
    _filter_chain: Option<Arc<FilterChain>>,
    receiver: Receiver<ConnectionCloseCommand>,
    close_timeout: Duration,
    close_timeout_total: Arc<AtomicU64>,
    terminated: Arc<AtomicBool>,
}

impl ConnectionCloseWorker {
    pub(crate) fn new(
        factory: Arc<dyn ConnectionFactory>,
        filter_chain: Option<Arc<FilterChain>>,
        receiver: Receiver<ConnectionCloseCommand>,
        close_timeout: Duration,
        close_timeout_total: Arc<AtomicU64>,
        terminated: Arc<AtomicBool>,
    ) -> Self {
        Self {
            _factory: factory,
            _filter_chain: filter_chain,
            receiver,
            close_timeout,
            close_timeout_total,
            terminated,
        }
    }

    pub(crate) fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(command) = self.receiver.recv().await {
                let mut holder = command.holder;
                // 出队立即释放容量；慢驱动关闭不会让待处理命令越积越多。
                drop(command.queue_permit);
                match tokio::time::timeout(self.close_timeout, holder.close_standard_connection())
                    .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::warn!(
                            %error,
                            connection_id = command.connection_id,
                            connection_age_millis = command.connection_age.as_millis(),
                            "close canonical connection failed"
                        );
                    }
                    Err(_) => {
                        self.close_timeout_total.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!(
                            connection_id = command.connection_id,
                            "close canonical connection timed out"
                        );
                    }
                }
            }
            self.terminated.store(true, Ordering::Release);
        })
    }
}
