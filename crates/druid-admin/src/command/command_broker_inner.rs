use std::collections::{HashMap, VecDeque};

use druid_metrics::protocol::{command, CommandAck, FrameHeader, ServerFrame};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tonic::Status;

pub(crate) type CommandKey = (String, String);
pub(crate) type CommandRecord = (
    Vec<u8>,
    command::Payload,
    Option<CommandAck>,
    watch::Sender<Option<CommandAck>>,
);
pub(crate) type LiveStream = (
    FrameHeader,
    mpsc::Sender<Result<ServerFrame, Status>>,
    CancellationToken,
);

/// [`super::CommandBroker`] 的锁内有界状态。
pub(crate) struct CommandBrokerInner {
    pub(crate) streams: HashMap<String, LiveStream>,
    pub(crate) commands: HashMap<CommandKey, CommandRecord>,
    pub(crate) order: VecDeque<CommandKey>,
}

impl CommandBrokerInner {
    /// 创建空 broker 状态。
    pub(crate) fn new() -> Self {
        Self {
            streams: HashMap::new(),
            commands: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// 淘汰最早的已完成结果，为新命令腾出一个有界槽位。
    pub(crate) fn make_room(&mut self, capacity: usize) -> bool {
        while self.commands.len() >= capacity {
            let completed = self.order.iter().position(|key| {
                self.commands
                    .get(key)
                    .is_some_and(|record| record.2.is_some())
            });
            let Some(index) = completed else {
                return false;
            };
            let key = self
                .order
                .remove(index)
                .expect("completed command index was validated");
            self.commands.remove(&key);
        }
        true
    }
}
