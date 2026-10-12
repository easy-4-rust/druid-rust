use super::DatabaseProfile;
use druid_rdbc::driver::ConnectionFactory;
use std::fmt;
use std::sync::Arc;

/// 已解析为 Druid 未池化 canonical 连接工厂的数据库产品驱动。
#[derive(Clone)]
pub struct ResolvedDatabaseDriver {
    profile: DatabaseProfile,
    url: String,
    factory: Arc<dyn ConnectionFactory>,
}

impl ResolvedDatabaseDriver {
    #[cfg(any(
        feature = "sqlx",
        feature = "jdbc-agent",
        feature = "duckdb-native",
        feature = "libsql-native",
        feature = "http-sql"
    ))]
    pub(crate) fn new(
        profile: DatabaseProfile,
        url: String,
        factory: Arc<dyn ConnectionFactory>,
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
    pub fn factory(&self) -> Arc<dyn ConnectionFactory> {
        Arc::clone(&self.factory)
    }
}

impl fmt::Debug for ResolvedDatabaseDriver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diagnostic_url = druid::sql::RdbcUrl::parse(&self.url).map_or_else(
            |_| "<redacted connection URL>".to_owned(),
            |url| url.redacted(),
        );
        formatter
            .debug_struct("ResolvedDatabaseDriver")
            .field("profile", &self.profile)
            .field("url", &diagnostic_url)
            .finish_non_exhaustive()
    }
}
