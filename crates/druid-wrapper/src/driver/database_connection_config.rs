use super::{DatabaseProfileId, DatabaseProfileIdError};
use std::collections::HashMap;

/// 解析数据库产品驱动所需的稳定连接配置。
#[derive(Clone)]
pub struct DatabaseConnectionConfig {
    profile_id: DatabaseProfileId,
    url: String,
    properties: HashMap<String, String>,
}

impl DatabaseConnectionConfig {
    /// 创建连接配置；凭据由驱动 URL 或现有 Druid connection properties 承载。
    pub fn new(
        profile_id: impl Into<String>,
        url: impl Into<String>,
    ) -> Result<Self, DatabaseProfileIdError> {
        Ok(Self {
            profile_id: DatabaseProfileId::new(profile_id)?,
            url: url.into(),
            properties: HashMap::new(),
        })
    }

    /// 设置 JDBC/驱动连接属性。
    #[must_use]
    pub fn property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(name.into(), value.into());
        self
    }

    /// 设置跨运行时统一用户名属性。
    #[must_use]
    pub fn user_name(self, user_name: impl Into<String>) -> Self {
        self.property("user", user_name)
    }

    /// 设置跨运行时统一密码属性；Debug 输出会隐藏该值。
    #[must_use]
    pub fn password(self, password: impl Into<String>) -> Self {
        self.property("password", password)
    }

    #[must_use]
    pub fn profile_id(&self) -> &DatabaseProfileId {
        &self.profile_id
    }
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 返回连接属性；调用方不得记录敏感值。
    #[must_use]
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }
}

impl std::fmt::Debug for DatabaseConnectionConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatabaseConnectionConfig")
            .field("profile_id", &self.profile_id)
            .field("url", &self.url)
            .field("property_names", &self.properties.keys())
            .finish()
    }
}
