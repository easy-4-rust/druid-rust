use std::sync::{Arc, Mutex};

use druid_metrics::protocol::{FrameHeader, ServerFrame};
use druid_metrics::ServiceIdentity;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::CommandBroker;

/// 一个 broker live stream 的精确 RAII 附着关系。
///
/// Rust-only 生命周期对象；每个副本都绑定同一个 broker、类型化身份与 sender，
/// Drop 只会注销仍指向该 sender 的当前 stream epoch。
#[derive(Clone)]
pub(crate) struct BrokerAttachment {
    broker: CommandBroker,
    service_identity: ServiceIdentity,
    sender: mpsc::Sender<ServerFrame>,
}

pub(crate) type BrokerAttachmentSlot = Arc<Mutex<Option<BrokerAttachment>>>;

impl BrokerAttachment {
    /// 原子注册 broker stream，并把可供 response Drop 使用的守卫副本写入共享槽位。
    pub(crate) fn attach(
        broker: CommandBroker,
        service_identity: ServiceIdentity,
        header: FrameHeader,
        sender: mpsc::Sender<ServerFrame>,
        cancellation: CancellationToken,
        slot: &BrokerAttachmentSlot,
    ) -> Result<Self, String> {
        broker.attach_stream(&service_identity, &header, sender.clone(), cancellation)?;
        let attachment = Self {
            broker,
            service_identity,
            sender,
        };
        *slot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(attachment.clone());
        Ok(attachment)
    }
}

impl Drop for BrokerAttachment {
    fn drop(&mut self) {
        self.broker
            .detach_stream(&self.service_identity, &self.sender);
    }
}
