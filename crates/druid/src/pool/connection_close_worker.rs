//! Rust 异步资源回收义务：受监管物理连接关闭 worker。
//!
//! Java `DruidDataSource` 能在调用线程同步关闭 JDBC Connection；Rust 的
//! `PhysicalConnection::close` 是 async，`Drop` 不能直接 await。本对象把脏
//! Drop 的关闭请求交给每个 `DruidPool` 唯一 worker，并由 pool 保存 JoinHandle。

use crate::core::{PhysicalConnection, PhysicalConnectionFactory};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedReceiver;

/// 顺序执行物理连接关闭的池内 worker。
///
/// `Some(connection)` 表示关闭一条连接，`None` 表示此前请求已经全部入队，
/// worker 完成 FIFO 排空后退出。
pub(crate) struct ConnectionCloseWorker {
    factory: Arc<dyn PhysicalConnectionFactory>,
    receiver: UnboundedReceiver<Option<Box<dyn PhysicalConnection>>>,
}

impl ConnectionCloseWorker {
    /// 创建尚未启动的 worker。
    pub(crate) fn new(
        factory: Arc<dyn PhysicalConnectionFactory>,
        receiver: UnboundedReceiver<Option<Box<dyn PhysicalConnection>>>,
    ) -> Self {
        Self { factory, receiver }
    }

    /// 启动 worker 并返回由 `DruidPool` 持有的任务句柄。
    pub(crate) fn spawn(mut self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(command) = self.receiver.recv().await {
                let Some(mut connection) = command else {
                    break;
                };
                let _ = self.factory.close(&mut connection).await;
            }
        })
    }
}
