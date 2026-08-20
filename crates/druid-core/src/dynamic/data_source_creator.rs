//! 对应 Java 类：`com.alibaba.druid.pool.ha.DataSourceCreator`。

use super::high_available_data_source::HighAvailableDataSourceInner;
use crate::core::{DruidError, PhysicalConnectionFactory, Pool};
use crate::pool::DruidPoolBuilder;
use crate::sql::RdbcUtils;
use std::sync::Arc;

/// 根据 HA 父数据源配置创建 Druid 子池。
///
/// 对应 Java: `com.alibaba.druid.pool.ha.DataSourceCreator`。工厂由外部通过
/// `DataSourceCreator::new` 注入，Core 不绑定具体驱动。
pub struct DataSourceCreator {
    factory_creator: Arc<dyn Fn(&str) -> Arc<dyn PhysicalConnectionFactory> + Send + Sync>,
}

impl DataSourceCreator {
    /// 使用指定工厂创建器构造 DataSourceCreator。
    pub fn new(
        factory_creator: Arc<dyn Fn(&str) -> Arc<dyn PhysicalConnectionFactory> + Send + Sync>,
    ) -> Self {
        Self { factory_creator }
    }

    /// 返回工厂创建器的克隆，供内部派生使用。
    pub(crate) fn clone_factory_creator(
        &self,
    ) -> Arc<dyn Fn(&str) -> Arc<dyn PhysicalConnectionFactory> + Send + Sync> {
        Arc::clone(&self.factory_creator)
    }

    /// 创建一个用于测试的 no-op creator；调用 `create` 会 panic。
    pub fn noop_for_test() -> Self {
        use crate::core::PhysicalConnection;
        struct NoopFactory;
        #[async_trait::async_trait]
        impl PhysicalConnectionFactory for NoopFactory {
            fn connection_url(&self) -> Option<&str> {
                None
            }
            async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
                panic!("noop factory should not be called")
            }
            async fn validate(
                &self,
                _connection: &mut Box<dyn PhysicalConnection>,
            ) -> Result<(), DruidError> {
                Ok(())
            }
        }
        Self::new(Arc::new(|_url| Arc::new(NoopFactory)))
    }

    /// 创建、初始化并返回一个命名子数据源。
    pub(crate) async fn create(
        &self,
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
        let factory = (self.factory_creator)(&driver_url);
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
            .driver_name(factory.driver_name().unwrap_or("unknown"))
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
