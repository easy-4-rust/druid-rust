use super::{DatabaseConnectionConfig, DriverRegistryError, DruidDriverRegistry};
#[cfg(feature = "jdbc-agent")]
use crate::jdbc_agent::JdbcAgentOptions;
use druid::pool::{DruidPool, DruidPoolBuilder};
use std::collections::HashMap;

/// 使用数据库产品档案配置现有 Druid native pool 的兼容建池入口。
pub struct DruidDatabasePoolBuilder {
    profile_id: String,
    url: String,
    name: Option<String>,
    properties: HashMap<String, String>,
    #[cfg(feature = "jdbc-agent")]
    jdbc_agent_options: Option<JdbcAgentOptions>,
    configure: Option<Box<dyn FnOnce(DruidPoolBuilder) -> DruidPoolBuilder + Send>>,
}

impl DruidDatabasePoolBuilder {
    /// 创建产品档案驱动的 Druid 建池器。
    #[must_use]
    pub fn new(profile_id: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            profile_id: profile_id.into(),
            url: url.into(),
            name: None,
            properties: HashMap::new(),
            #[cfg(feature = "jdbc-agent")]
            jdbc_agent_options: None,
            configure: None,
        }
    }

    /// 设置跨运行时连接用户名。
    #[must_use]
    pub fn user_name(mut self, user_name: impl Into<String>) -> Self {
        self.properties.insert("user".to_owned(), user_name.into());
        self
    }

    /// 设置跨运行时连接密码。
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.properties
            .insert("password".to_owned(), password.into());
        self
    }

    /// 设置驱动连接属性。
    #[must_use]
    pub fn property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// 显式启用 Druid JDBC Agent 运行时。
    #[cfg(feature = "jdbc-agent")]
    #[must_use]
    pub fn jdbc_agent(mut self, options: JdbcAgentOptions) -> Self {
        self.jdbc_agent_options = Some(options);
        self
    }

    /// 设置数据源名称。
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 在驱动工厂注入前配置现有 `DruidPoolBuilder`。
    #[must_use]
    pub fn pool(
        mut self,
        configure: impl FnOnce(DruidPoolBuilder) -> DruidPoolBuilder + Send + 'static,
    ) -> Self {
        self.configure = Some(Box::new(configure));
        self
    }

    /// 解析驱动并构建 canonical Druid native pool。
    pub async fn build(self) -> Result<DruidPool, DriverRegistryError> {
        #[cfg(feature = "jdbc-agent")]
        let mut registry = DruidDriverRegistry::builtin()?;
        #[cfg(not(feature = "jdbc-agent"))]
        let registry = DruidDriverRegistry::builtin()?;
        #[cfg(feature = "jdbc-agent")]
        if let Some(options) = self.jdbc_agent_options {
            registry = registry.with_jdbc_agent(options);
        }
        let mut config = DatabaseConnectionConfig::new(self.profile_id, self.url)?;
        for (name, value) in self.properties {
            config = config.property(name, value);
        }
        let resolved = registry.resolve(&config)?;
        let profile = resolved.profile();
        let mut builder = DruidPool::builder()
            .name(self.name.unwrap_or_else(|| profile.id().to_string()))
            .driver_name(format!("{}:{}", profile.provider_id(), profile.id()))
            .db_type_name(profile.db_type().as_str())
            .url(resolved.url())
            .factory(resolved.factory());
        if let Some(validation_query) = profile.validation_query() {
            builder = builder.validation_query(validation_query);
        }
        if let Some(configure) = self.configure {
            builder = configure(builder);
        }
        builder.build().await.map_err(Into::into)
    }
}
