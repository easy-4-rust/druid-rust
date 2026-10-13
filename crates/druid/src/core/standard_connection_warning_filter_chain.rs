use super::{BeforeFilter, DruidError, SqlWarning};
use druid_rdbc::connection::Connection;
use std::sync::Arc;

/// 在标准连接上按注册顺序分派警告读取与清除过滤器。
/// 对应 Java: FilterChainImpl#connection_getWarnings/connection_clearWarnings。
pub struct StandardConnectionWarningFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    connection: &'a mut dyn Connection,
}

impl<'a> StandardConnectionWarningFilterChain<'a> {
    /// 从首个过滤器开始构造调用链；参数为过滤器序列和标准连接。
    pub(crate) fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        connection: &'a mut dyn Connection,
    ) -> Self {
        Self {
            filters,
            position: 0,
            connection,
        }
    }

    /// 调用下一个过滤器；到达链尾时返回标准连接的警告链。
    pub async fn connection_get_warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.connection_get_warnings(self).await
        } else {
            self.connection.warnings().await.map_err(DruidError::from)
        }
    }

    /// 调用下一个过滤器；到达链尾时清除标准连接的警告链。
    pub async fn connection_clear_warnings(&mut self) -> Result<(), DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.connection_clear_warnings(self).await
        } else {
            self.connection
                .clear_warnings()
                .await
                .map_err(DruidError::from)
        }
    }
}
