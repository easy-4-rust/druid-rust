use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use druid_metrics::protocol::ServerFrame;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::Stream;
use tonic::Status;

use super::BrokerAttachmentSlot;

/// 拥有单个 Admin bidi stream 处理任务的 response stream。
///
/// Rust-only 生命周期对象；Tonic 丢弃 response stream 时同步 abort 未结束任务，
/// 正常 EOF/shutdown 则由 receiver 关闭并保留已完成 join 状态。
pub struct OwnedConnectStream {
    response_receiver: mpsc::Receiver<Result<ServerFrame, Status>>,
    command_receiver: mpsc::Receiver<ServerFrame>,
    task: Option<JoinHandle<()>>,
    attachment: BrokerAttachmentSlot,
    terminated: bool,
    protocol_started: bool,
    prefer_response: bool,
}

impl OwnedConnectStream {
    /// 绑定有界 response receiver 与唯一处理任务。
    pub(crate) fn new(
        response_receiver: mpsc::Receiver<Result<ServerFrame, Status>>,
        command_receiver: mpsc::Receiver<ServerFrame>,
        task: JoinHandle<()>,
        attachment: BrokerAttachmentSlot,
    ) -> Self {
        Self {
            response_receiver,
            command_receiver,
            task: Some(task),
            attachment,
            terminated: false,
            protocol_started: false,
            prefer_response: true,
        }
    }

    fn detach_attachment(&self) {
        self.attachment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}

impl Stream for OwnedConnectStream {
    type Item = Result<ServerFrame, Status>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.terminated {
            return Poll::Ready(None);
        }
        if let Some(task) = self.task.as_mut() {
            match Pin::new(task).poll(context) {
                Poll::Ready(Ok(())) => {
                    self.task.take();
                    self.detach_attachment();
                    self.command_receiver.close();
                }
                Poll::Ready(Err(_)) => {
                    self.task.take();
                    self.detach_attachment();
                    self.response_receiver.close();
                    self.command_receiver.close();
                    self.terminated = true;
                    return Poll::Ready(Some(Err(Status::internal(
                        "ingest stream processing task failed",
                    ))));
                }
                Poll::Pending => {}
            }
        }
        if !self.protocol_started {
            return match Pin::new(&mut self.response_receiver).poll_recv(context) {
                Poll::Ready(Some(response)) => {
                    self.protocol_started = true;
                    Poll::Ready(Some(response))
                }
                Poll::Ready(None) if self.task.is_none() => {
                    self.terminated = true;
                    Poll::Ready(None)
                }
                Poll::Ready(None) | Poll::Pending => Poll::Pending,
            };
        }

        if self.prefer_response {
            if let Poll::Ready(Some(response)) =
                Pin::new(&mut self.response_receiver).poll_recv(context)
            {
                self.prefer_response = false;
                return Poll::Ready(Some(response));
            }
            if let Poll::Ready(Some(command)) =
                Pin::new(&mut self.command_receiver).poll_recv(context)
            {
                self.prefer_response = true;
                return Poll::Ready(Some(Ok(command)));
            }
        } else {
            if let Poll::Ready(Some(command)) =
                Pin::new(&mut self.command_receiver).poll_recv(context)
            {
                self.prefer_response = true;
                return Poll::Ready(Some(Ok(command)));
            }
            if let Poll::Ready(Some(response)) =
                Pin::new(&mut self.response_receiver).poll_recv(context)
            {
                self.prefer_response = false;
                return Poll::Ready(Some(response));
            }
        }
        if self.task.is_none()
            && self.response_receiver.is_closed()
            && self.command_receiver.is_closed()
            && self.response_receiver.is_empty()
            && self.command_receiver.is_empty()
        {
            self.terminated = true;
            Poll::Ready(None)
        } else {
            Poll::Pending
        }
    }
}

impl Drop for OwnedConnectStream {
    fn drop(&mut self) {
        self.detach_attachment();
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use druid_metrics::protocol::FrameHeader;
    use tokio::sync::Notify;
    use tokio_stream::StreamExt as _;
    use tokio_util::sync::CancellationToken;

    use super::{BrokerAttachmentSlot, OwnedConnectStream};
    use crate::ingest::BrokerAttachment;
    use crate::CommandBroker;

    #[tokio::test]
    async fn processing_panic_is_observed_while_response_sender_remains_open_exactly_once() {
        let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
        let (_command_tx, command_rx) = tokio::sync::mpsc::channel(1);
        let task = tokio::spawn(async {
            panic!("owned connect processing panic probe");
        });
        let mut stream = OwnedConnectStream::new(
            response_rx,
            command_rx,
            task,
            std::sync::Arc::new(std::sync::Mutex::new(None)),
        );

        let failure = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .expect("processing panic must wake the owned response stream")
            .expect("panic produces one terminal status");
        assert_eq!(
            failure
                .expect_err("processing panic is an internal status")
                .code(),
            tonic::Code::Internal
        );
        assert!(
            stream.next().await.is_none(),
            "processing panic must terminate after exactly one internal status"
        );
        drop(response_tx);
    }

    #[tokio::test]
    async fn response_stream_drop_detaches_broker_before_aborted_task_is_joined() {
        let broker = CommandBroker::new(1, Duration::from_secs(1));
        let (response_tx, response_rx) = tokio::sync::mpsc::channel(1);
        let (command_tx, command_rx) = tokio::sync::mpsc::channel(1);
        let attachment: BrokerAttachmentSlot = Arc::new(Mutex::new(None));
        let task_attachment = Arc::clone(&attachment);
        let task_broker = broker.clone();
        let attached = Arc::new(Notify::new());
        let task_attached = Arc::clone(&attached);
        let task = tokio::spawn(async move {
            let _guard = BrokerAttachment::attach(
                task_broker,
                FrameHeader {
                    protocol_version: 1,
                    service_name: "orders".to_owned(),
                    instance_id: "instance-drop".to_owned(),
                    boot_id: "boot-drop".to_owned(),
                    stream_epoch: 7,
                    sequence: 0,
                    emitted_at_unix_ms: 1,
                },
                command_tx,
                CancellationToken::new(),
                &task_attachment,
            )
            .expect("attach response-drop probe");
            task_attached.notify_one();
            std::future::pending::<()>().await;
        });
        let stream = OwnedConnectStream::new(response_rx, command_rx, task, attachment);
        attached.notified().await;
        assert_eq!(broker.live_stream_count(), 1);

        drop(stream);
        assert_eq!(
            broker.live_stream_count(),
            0,
            "response Drop must detach synchronously before task cancellation is polled"
        );
        drop(response_tx);
    }
}
