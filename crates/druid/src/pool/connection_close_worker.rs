//! Rust 异步资源回收义务：受监管物理连接关闭 worker。
//!
//! Java `DruidDataSource` 能在调用线程同步关闭 RDBC Connection；Rust 的
//! `PhysicalConnection::close` 是 async，`Drop` 不能直接 await。本对象把脏
//! Drop 的关闭请求交给每个 `DruidPool` 唯一 worker，并由 pool 保存 `JoinHandle`。

use crate::core::{FilterChain, PhysicalConnection, PhysicalConnectionFactory};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

/// 一次真实物理连接关闭命令。
pub(crate) struct ConnectionCloseCommand {
    /// Druid 物理连接 ID。
    pub(crate) connection_id: u64,
    /// 从物理连接建立完成到提交关闭命令的寿命。
    pub(crate) physical_age: Duration,
    /// 待关闭的原始驱动连接。
    pub(crate) connection: Box<dyn PhysicalConnection>,
}

/// 顺序执行物理连接关闭的池内 worker。
///
/// `Some(command)` 表示关闭一条连接，`None` 表示此前请求已经全部入队，
/// worker 完成 FIFO 排空后退出。
pub(crate) struct ConnectionCloseWorker {
    factory: Arc<dyn PhysicalConnectionFactory>,
    filter_chain: Option<Arc<FilterChain>>,
    receiver: UnboundedReceiver<Option<ConnectionCloseCommand>>,
}

impl ConnectionCloseWorker {
    /// 创建尚未启动的 worker。
    pub(crate) fn new(
        factory: Arc<dyn PhysicalConnectionFactory>,
        filter_chain: Option<Arc<FilterChain>>,
        receiver: UnboundedReceiver<Option<ConnectionCloseCommand>>,
    ) -> Self {
        Self {
            factory,
            filter_chain,
            receiver,
        }
    }

    /// 启动 worker 并返回由 `DruidPool` 持有的任务句柄。
    pub(crate) fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(command) = self.receiver.recv().await {
                let Some(mut command) = command else {
                    break;
                };
                let result = match &self.filter_chain {
                    Some(filter_chain) if !filter_chain.is_empty() => {
                        filter_chain
                            .physical_connection_close(
                                self.factory.as_ref(),
                                &mut command.connection,
                                command.connection_id,
                                command.physical_age,
                            )
                            .await
                    }
                    _ => self.factory.close(&mut command.connection).await,
                };
                if let Err(error) = result {
                    tracing::warn!(
                        %error,
                        connection_id = command.connection_id,
                        "close physical connection failed"
                    );
                }
            }
        })
    }
}
