use druid::core::{DruidError, PhysicalConnectionFactory};
use druid::pool::DruidDataSource;
use std::collections::HashMap;
use std::sync::Arc;

use super::BasicDataSourceFactory;

/// Apache DBCP 1 managed 数据源工厂。
///
/// 对应 Java: `org.apache.commons.dbcp.ManagedBasicDataSourceFactory`。Java
/// 仅覆写构造类型；Rust 的 managed 能力已由 `DruidDataSource` 实现。
#[derive(Debug, Default, Clone, Copy)]
pub struct ManagedBasicDataSourceFactory;

impl ManagedBasicDataSourceFactory {
    /// 使用 Druid 内置 Toasty factory 创建 managed 数据源。
    pub async fn create_data_source(
        properties: &HashMap<String, String>,
    ) -> Result<DruidDataSource, DruidError> {
        BasicDataSourceFactory::create_data_source(properties).await
    }

    /// 使用扩展物理连接 factory 创建 managed 数据源。
    pub async fn create_data_source_with_factory(
        properties: &HashMap<String, String>,
        factory: Arc<dyn PhysicalConnectionFactory>,
        driver_name: impl Into<String>,
    ) -> Result<DruidDataSource, DruidError> {
        BasicDataSourceFactory::create_data_source_with_factory(properties, factory, driver_name)
            .await
    }
}
