use crate::driver::{ConnectionFactory, ConnectionProperties};
use std::sync::Arc;

/// 按已选定的 provider 创建标准连接工厂；产品档案及 URL 校验由调用方负责。
/// 对应 Java: `javax.sql.DataSource` 的连接来源职责，此接口为 Rust 扩展。
pub trait ConnectionFactoryProvider: Send + Sync + 'static {
    /// 使用驱动 URL、连接属性和可选验证语句创建工厂；此方法不执行连接 I/O。
    fn factory(
        &self,
        url: &str,
        properties: &ConnectionProperties,
        validation_query: Option<&str>,
    ) -> Arc<dyn ConnectionFactory>;
}

impl<F> ConnectionFactoryProvider for F
where
    F: Fn(&str, &ConnectionProperties, Option<&str>) -> Arc<dyn ConnectionFactory>
        + Send
        + Sync
        + 'static,
{
    fn factory(
        &self,
        url: &str,
        properties: &ConnectionProperties,
        validation_query: Option<&str>,
    ) -> Arc<dyn ConnectionFactory> {
        self(url, properties, validation_query)
    }
}
