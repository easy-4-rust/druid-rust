//! 对应 Java 类：`com.alibaba.druid.pool.ha.DataSourceCreator`。

use super::high_available_data_source::HighAvailableDataSourceInner;
use crate::core::{DruidError, Pool};
use crate::pool::DruidPoolBuilder;
use crate::sql::RdbcUtils;
use crate::toasty::ToastyConnectionFactory;
use std::sync::Arc;

/// 根据 HA 父数据源配置创建 Druid 子池。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.DataSourceCreator`。Rust 内置实现使用
/// Toasty 的未池化 Driver SPI 构造物理连接，仍由 `DruidPool` 作为唯一连接池。
pub struct DataSourceCreator;

impl DataSourceCreator {
    /// 创建、初始化并返回一个命名子数据源。
    pub(crate) async fn create(
        node_name: &str,
        url: Option<&str>,
        username: Option<&str>,
        password: Option<&str>,
        high_available_data_source: &Arc<HighAvailableDataSourceInner>,
    ) -> Result<Arc<dyn Pool>, DruidError> {
        let raw_url = url.ok_or_else(|| {
            DruidError::InvalidArgument(format!("{node_name}.url must not be empty"))
        })?;
        let rust_url = RdbcUtils::to_rust_url(raw_url).ok_or(DruidError::UnsupportedOperation {
            operation: "ha_node_driver_adapter_required",
        })?;
        let driver_url = Self::apply_credentials(rust_url.as_ref(), username, password)?;
        let factory = Arc::new(ToastyConnectionFactory::new(&driver_url).await?);
        let config = high_available_data_source.config.read().clone();
        let mut connection_properties = config.connect_properties.clone();
        if let Some(username) = username {
            connection_properties.insert("user".to_owned(), username.to_owned());
        }
        if let Some(password) = password {
            connection_properties.insert("password".to_owned(), password.to_owned());
        }

        let mut builder = DruidPoolBuilder::new()
            .name(format!("{node_name}-{}", uuid::Uuid::new_v4()))
            .driver_name(factory.driver_name())
            .url(raw_url)
            .raw_url(driver_url)
            .factory(factory)
            .connection_properties(connection_properties)
            .initial_size(config.initial_size)
            .max_open(config.max_active)
            .min_idle(config.min_idle)
            .max_idle(config.max_active)
            .acquire_timeout(config.max_wait)
            .test_on_borrow(config.test_on_borrow)
            .test_on_return(config.test_on_return)
            .test_while_idle(config.test_while_idle)
            .validation_query_timeout(config.validation_query_timeout)
            .query_timeout(config.query_timeout)
            .transaction_query_timeout(config.transaction_query_timeout)
            .time_between_eviction_runs(config.time_between_eviction_runs)
            .idle_timeout(config.min_evictable_idle_time)
            .max_evictable_idle_time(config.max_evictable_idle_time)
            .time_between_connect_error(config.time_between_connect_error)
            .remove_abandoned(config.remove_abandoned)
            .remove_abandoned_timeout(config.remove_abandoned_timeout)
            .log_abandoned(config.log_abandoned)
            .pool_prepared_statements(config.pool_prepared_statements)
            .share_prepared_statements(config.share_prepared_statements)
            .max_pool_prepared_statements_per_connection(
                config.max_pool_prepared_statement_per_connection_size,
            );
        if let Some(validation_query) = config.validation_query {
            builder = builder.validation_query(validation_query);
        }
        if let Some(physical_timeout) = config.physical_timeout {
            builder = builder.physical_connection_timeout(physical_timeout);
        }
        if let Some(filters) = config.filters.as_deref() {
            builder.set_filters(Some(filters))?;
        }

        let data_source = Arc::new(builder.build_data_source().await?);
        data_source.init().await?;
        Ok(data_source)
    }

    fn apply_credentials(
        raw_url: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> Result<String, DruidError> {
        if username.is_none() && password.is_none() {
            return Ok(raw_url.to_owned());
        }
        let mut parsed = url::Url::parse(raw_url).map_err(|error| {
            DruidError::InvalidArgument(format!("invalid database URL for credentials: {error}"))
        })?;
        if parsed.scheme().eq_ignore_ascii_case("sqlite") {
            return Ok(raw_url.to_owned());
        }
        if let Some(username) = username {
            parsed.set_username(username).map_err(|()| {
                DruidError::InvalidArgument("database URL cannot carry username".to_owned())
            })?;
        }
        if let Some(password) = password {
            parsed.set_password(Some(password)).map_err(|()| {
                DruidError::InvalidArgument("database URL cannot carry password".to_owned())
            })?;
        }
        Ok(parsed.into())
    }
}
