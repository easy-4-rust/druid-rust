use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use druid_metrics::protocol::ServerFrame;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::Stream;
use tonic::Status;

/// 拥有单个 Admin bidi stream 处理任务的 response stream。
///
/// Rust-only 生命周期对象；Tonic 丢弃 response stream 时同步 abort 未结束任务，
/// 正常 EOF/shutdown 则由 receiver 关闭并保留已完成 join 状态。
pub struct OwnedConnectStream {
    receiver: mpsc::Receiver<Result<ServerFrame, Status>>,
    task: Option<JoinHandle<()>>,
}

impl OwnedConnectStream {
    /// 绑定有界 response receiver 与唯一处理任务。
    pub(crate) fn new(
        receiver: mpsc::Receiver<Result<ServerFrame, Status>>,
        task: JoinHandle<()>,
    ) -> Self {
        Self {
            receiver,
            task: Some(task),
        }
    }
}

impl Stream for OwnedConnectStream {
    type Item = Result<ServerFrame, Status>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.receiver).poll_recv(context) {
            Poll::Ready(Some(response)) => Poll::Ready(Some(response)),
            Poll::Pending => Poll::Pending,
            Poll::Ready(None) => {
                let Some(task) = self.task.as_mut() else {
                    return Poll::Ready(None);
                };
                match Pin::new(task).poll(context) {
                    Poll::Pending => Poll::Pending,
                    Poll::Ready(Ok(())) => {
                        self.task.take();
                        Poll::Ready(None)
                    }
                    Poll::Ready(Err(_)) => {
                        self.task.take();
                        Poll::Ready(Some(Err(Status::internal(
                            "ingest stream processing task failed",
                        ))))
                    }
                }
            }
        }
    }
}

impl Drop for OwnedConnectStream {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
