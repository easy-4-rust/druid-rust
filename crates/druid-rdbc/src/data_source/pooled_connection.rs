use crate::connection::Connection;
use crate::data_source::{ConnectionEventListener, StatementEventListener};
use crate::error::RdbcError;
use std::sync::Arc;

/// `javax.sql.PooledConnection` 的 logical-connection source 标准。
#[async_trait::async_trait]
pub trait PooledConnection: Send {
    /// 创建一个 owned logical connection handle。
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError>;

    /// 注册连接关闭与连接错误监听器。
    ///
    /// 默认实现报告 `0A000`，以保持既有外部实现的源兼容；支持池化事件的实现应保存监听器，
    /// 并在逻辑连接归还或连接失效时调用对应回调。重复注册允许存在，移除时按 Arc 指针身份
    /// 删除一个匹配项，对齐 Java 默认对象相等语义下的 `CopyOnWriteArrayList` 行为。
    fn add_connection_event_listener(
        &mut self,
        _listener: Arc<dyn ConnectionEventListener>,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported(
            "pooled_connection_add_connection_event_listener",
        ))
    }

    /// 移除一个连接事件监听器；返回是否移除了一个注册项。
    fn remove_connection_event_listener(
        &mut self,
        _listener: &Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "pooled_connection_remove_connection_event_listener",
        ))
    }

    /// 注册 PreparedStatement 生命周期监听器；重复注册允许存在。
    fn add_statement_event_listener(
        &mut self,
        _listener: Arc<dyn StatementEventListener>,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported(
            "pooled_connection_add_statement_event_listener",
        ))
    }

    /// 移除一个语句事件监听器；返回是否移除了一个注册项。
    fn remove_statement_event_listener(
        &mut self,
        _listener: &Arc<dyn StatementEventListener>,
    ) -> Result<bool, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "pooled_connection_remove_statement_event_listener",
        ))
    }

    /// 关闭该 pooled connection owner。
    async fn close(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("pooled_connection_close"))
    }
}
