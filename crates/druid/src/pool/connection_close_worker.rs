//! Supervised async close worker for canonical holder resources.

use crate::core::{ConnectionFactory, DruidConnectionHolder, FilterChain};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

pub(crate) struct ConnectionCloseCommand {
    pub(crate) connection_id: u64,
    pub(crate) connection_age: Duration,
    pub(crate) holder: Box<DruidConnectionHolder>,
}

pub(crate) struct ConnectionCloseWorker {
    _factory: Arc<dyn ConnectionFactory>,
    _filter_chain: Option<Arc<FilterChain>>,
    receiver: UnboundedReceiver<Option<ConnectionCloseCommand>>,
}

impl ConnectionCloseWorker {
    pub(crate) fn new(
        factory: Arc<dyn ConnectionFactory>,
        filter_chain: Option<Arc<FilterChain>>,
        receiver: UnboundedReceiver<Option<ConnectionCloseCommand>>,
    ) -> Self {
        Self {
            _factory: factory,
            _filter_chain: filter_chain,
            receiver,
        }
    }

    pub(crate) fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(command) = self.receiver.recv().await {
                let Some(command) = command else {
                    break;
                };
                let mut holder = command.holder;
                if let Err(error) = holder.close_standard_connection().await {
                    tracing::warn!(
                        %error,
                        connection_id = command.connection_id,
                        connection_age_millis = command.connection_age.as_millis(),
                        "close canonical connection failed"
                    );
                }
            }
        })
    }
}
