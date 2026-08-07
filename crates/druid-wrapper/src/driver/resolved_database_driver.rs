use super::DatabaseProfile;
use druid::core::PhysicalConnectionFactory;
use std::fmt;
use std::sync::Arc;

/// 已解析为 Druid 未池化物理连接工厂的数据库产品驱动。
#[derive(Clone)]
pub struct ResolvedDatabaseDriver {
    profile: DatabaseProfile,
    url: String,
    factory: Arc<dyn PhysicalConnectionFactory>,
}

impl ResolvedDatabaseDriver {
    pub(crate) fn new(
        profile: DatabaseProfile,
        url: String,
        factory: Arc<dyn PhysicalConnectionFactory>,
    ) -> Self {
        Self {
            profile,
            url,
            factory,
        }
    }

    #[must_use]
    pub fn profile(&self) -> &DatabaseProfile {
        &self.profile
    }
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
    #[must_use]
    pub fn factory(&self) -> Arc<dyn PhysicalConnectionFactory> {
        Arc::clone(&self.factory)
    }
}

impl fmt::Debug for ResolvedDatabaseDriver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolvedDatabaseDriver")
            .field("profile", &self.profile)
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}
