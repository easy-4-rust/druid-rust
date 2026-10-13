use crate::data_source::{PooledConnection, XaError, XaResource};

/// `javax.sql.XAConnection` 的 standard extension。
pub trait XaConnection: PooledConnection {
    /// 返回实现提供的 XA resource identity。
    fn xa_resource_name(&self) -> &str;

    /// 返回由此 XA 连接 owner 管理的可操作资源。
    ///
    /// 默认明确报告 XA 不受支持，以保持既有外部实现的源兼容。支持 XA 的实现必须返回与此
    /// owner 生命周期绑定的资源，不得只用 `xa_resource_name` 冒充协议能力。
    fn xa_resource(&mut self) -> Result<&mut (dyn XaResource + '_), XaError> {
        Err(XaError::feature_not_supported("xa_connection_xa_resource"))
    }
}
