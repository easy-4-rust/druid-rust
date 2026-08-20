use super::{JdbcAgentConnection, JdbcAgentOptions};
use druid_core::core::{DruidError, PhysicalConnection, PhysicalConnectionFactory};
use std::collections::HashMap;

/// 每次创建一个未池化 JDBC Agent 物理连接的 Druid 工厂。
#[derive(Clone)]
pub struct JdbcAgentConnectionFactory {
    url: String,
    validation_query: Option<String>,
    user_name: Option<String>,
    password: Option<String>,
    properties: HashMap<String, String>,
    options: JdbcAgentOptions,
}

impl JdbcAgentConnectionFactory {
    /// 创建显式配置 Agent 进程的未池化连接工厂。
    #[must_use]
    pub fn new(
        url: impl Into<String>,
        validation_query: Option<String>,
        options: JdbcAgentOptions,
    ) -> Self {
        Self {
            url: url.into(),
            validation_query,
            user_name: None,
            password: None,
            properties: HashMap::new(),
            options,
        }
    }

    /// 设置 JDBC 用户名。
    #[must_use]
    pub fn user_name(mut self, user_name: impl Into<String>) -> Self {
        self.user_name = Some(user_name.into());
        self
    }

    /// 设置 JDBC 密码；Debug 输出不会包含该值。
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// 设置额外 JDBC 连接属性。
    #[must_use]
    pub fn property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    async fn create_with_properties(
        &self,
        properties: HashMap<String, String>,
    ) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let connection = JdbcAgentConnection::connect(
            &self.url,
            self.validation_query.as_deref(),
            properties,
            self.options.clone(),
        )
        .await?;
        Ok(Box::new(connection))
    }

    fn merged_properties(
        &self,
        one_shot: Option<&HashMap<String, String>>,
    ) -> HashMap<String, String> {
        let mut properties = self.properties.clone();
        if let Some(one_shot) = one_shot {
            properties.extend(one_shot.clone());
        }
        if let Some(user_name) = &self.user_name {
            properties
                .entry("user".to_owned())
                .or_insert_with(|| user_name.clone());
        }
        if let Some(password) = &self.password {
            properties
                .entry("password".to_owned())
                .or_insert_with(|| password.clone());
        }
        properties
    }
}

impl std::fmt::Debug for JdbcAgentConnectionFactory {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentConnectionFactory")
            .field("url", &self.url)
            .field("validation_query", &self.validation_query)
            .field("user_name", &self.user_name)
            .field("has_password", &self.password.is_some())
            .field("property_names", &self.properties.keys())
            .field("options", &self.options)
            .finish()
    }
}

#[async_trait::async_trait]
impl PhysicalConnectionFactory for JdbcAgentConnectionFactory {
    fn connection_url(&self) -> Option<&str> {
        Some(&self.url)
    }

    fn user_name(&self) -> Option<&str> {
        self.user_name.as_deref()
    }

    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        self.create_with_properties(self.merged_properties(None))
            .await
    }

    async fn create_info_with_properties(
        &self,
        properties: &HashMap<String, String>,
    ) -> Result<druid_core::core::PhysicalConnectionInfo, DruidError> {
        let started_at = std::time::Instant::now();
        let connection = self
            .create_with_properties(self.merged_properties(Some(properties)))
            .await?;
        Ok(druid_core::core::PhysicalConnectionInfo::connected(
            connection, started_at,
        ))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }
}
