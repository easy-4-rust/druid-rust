use druid::core::{DruidError, PhysicalConnectionFactory};
use druid::pool::{DruidDataSource, DruidDataSourceFactory};
use std::collections::HashMap;
use std::sync::Arc;

/// Apache DBCP 2 数据源工厂。
///
/// 对应 Java: `org.apache.commons.dbcp2.BasicDataSourceFactory`。
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicDataSourceFactory;

impl BasicDataSourceFactory {
    /// 使用 Druid 内置 Toasty factory 创建数据源。
    pub async fn create_data_source(
        properties: &HashMap<String, String>,
    ) -> Result<DruidDataSource, DruidError> {
        DruidDataSourceFactory::create_data_source(properties).await
    }

    /// 使用扩展物理连接 factory 创建数据源。
    pub async fn create_data_source_with_factory(
        properties: &HashMap<String, String>,
        factory: Arc<dyn PhysicalConnectionFactory>,
        driver_name: impl Into<String>,
    ) -> Result<DruidDataSource, DruidError> {
        DruidDataSourceFactory::create_data_source_with_factory(properties, factory, driver_name)
            .await
    }
}
