use crate::data_source::PooledConnection;

/// `javax.sql.XAConnection` 的 standard extension。
pub trait XaConnection: PooledConnection {
    /// 返回实现提供的 XA resource identity。
    fn xa_resource_name(&self) -> &str;
}
