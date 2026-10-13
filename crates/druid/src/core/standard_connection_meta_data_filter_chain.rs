use super::{BeforeFilter, DruidError};
use druid_rdbc::connection::Connection;
use druid_rdbc::metadata::DatabaseMetaData;
use std::sync::Arc;

/// 在标准连接上包围完整 metadata 获取，允许过滤器替换对象或短路驱动。
/// 对应 Java: FilterChainImpl#connection_getMetaData。
pub struct StandardConnectionMetaDataFilterChain<'a> {
    filters: &'a [Arc<dyn BeforeFilter>],
    position: usize,
    connection: &'a mut dyn Connection,
    connection_id: u64,
}

impl<'a> StandardConnectionMetaDataFilterChain<'a> {
    /// 从首个过滤器开始构造调用链；参数为过滤器、标准连接与逻辑连接 ID。
    pub(crate) fn new(
        filters: &'a [Arc<dyn BeforeFilter>],
        connection: &'a mut dyn Connection,
        connection_id: u64,
    ) -> Self {
        Self {
            filters,
            position: 0,
            connection,
            connection_id,
        }
    }

    /// 返回当前逻辑连接 ID；无参数，不转移连接所有权。
    pub const fn connection_id(&self) -> u64 {
        self.connection_id
    }

    /// 继续到下一个过滤器或标准驱动；返回完整 metadata 对象及原始错误。
    pub async fn connection_get_meta_data(
        &mut self,
    ) -> Result<Box<dyn DatabaseMetaData>, DruidError> {
        if let Some(filter) = self.filters.get(self.position).cloned() {
            self.position += 1;
            filter.standard_connection_get_meta_data(self).await
        } else {
            self.connection
                .database_meta_data()
                .await
                .map_err(DruidError::from)
        }
    }
}
