use crate::toasty::ToastyConnectionFactory;
use druid::pool::{DruidDataSource, DruidDataSourceFactory};
use druid::sql::RdbcUtils;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use std::collections::HashMap;
use std::sync::Arc;

/// Apache DBCP 1 数据源工厂。
///
/// 对应 Java: `org.apache.commons.dbcp.BasicDataSourceFactory`。属性解析完全
/// 委托 canonical `DruidDataSourceFactory`，不复制池状态。
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicDataSourceFactory;

impl BasicDataSourceFactory {
    /// 使用 Druid 内置 Toasty factory 创建数据源。
    pub async fn create_data_source(
        properties: &HashMap<String, String>,
    ) -> Result<DruidDataSource, RdbcError> {
        let properties = DruidDataSourceFactory::resolve_config_properties(properties).await?;
        let url = properties
            .get("url")
            .ok_or_else(|| RdbcError::InvalidArgument("url is required".to_owned()))?;
        let rust_url = RdbcUtils::to_rust_url(url).ok_or_else(|| {
            RdbcError::InvalidArgument(format!("Toasty does not support RDBC URL `{url}`"))
        })?;
        let factory: Arc<dyn ConnectionFactory> =
            Arc::new(ToastyConnectionFactory::new(rust_url.as_ref()).await?);
        let driver_name = factory
            .connection_url()
            .map_or_else(|| "toasty".to_owned(), str::to_owned);
        DruidDataSourceFactory::create_data_source_with_factory(&properties, factory, driver_name)
            .await
            .map_err(RdbcError::from)
    }

    /// 使用扩展 canonical connection factory 创建数据源。
    pub async fn create_data_source_with_factory(
        properties: &HashMap<String, String>,
        factory: Arc<dyn ConnectionFactory>,
        driver_name: impl Into<String>,
    ) -> Result<DruidDataSource, RdbcError> {
        DruidDataSourceFactory::create_data_source_with_factory(properties, factory, driver_name)
            .await
            .map_err(RdbcError::from)
    }
}
