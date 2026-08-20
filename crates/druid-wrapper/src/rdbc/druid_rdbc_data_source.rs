use std::any::Any;

use async_trait::async_trait;
use druid_core::core::{DruidError, DruidPooledConnection, SqlException, Wrapper};
use druid_core::pool::DruidDataSource;
use druid_core::sql::{CommonDataSource, DataSource};

use crate::driver::{DriverRegistryError, DruidDatabasePoolBuilder};

/// 在 wrapper 层实现的 Druid RDBC 数据源。
///
/// 对应 Java: `javax.sql.DataSource`。底层只持有一个 canonical
/// [`DruidDataSource`]，显式凭据仅用于校验，禁止每次调用创建新连接池。
pub struct DruidRdbcDataSource {
    inner: DruidDataSource,
    credentials: Option<(String, String)>,
}

impl DruidRdbcDataSource {
    /// 从统一 RDBC URL 构建目录驱动的 canonical Druid DataSource。
    pub async fn connect(rdbc_url: impl Into<String>) -> Result<Self, DriverRegistryError> {
        let data_source = DruidDatabasePoolBuilder::from_rdbc_url(rdbc_url)?
            .build_data_source()
            .await?;
        Ok(Self::new(data_source))
    }

    /// 包装现有 Druid 数据源。
    #[must_use]
    pub fn new(inner: DruidDataSource) -> Self {
        Self {
            inner,
            credentials: None,
        }
    }

    /// 包装现有数据源并声明其固定凭据。
    #[must_use]
    pub fn with_credentials(
        inner: DruidDataSource,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            credentials: Some((username.into(), password.into())),
        }
    }

    /// 返回 canonical Druid 数据源。
    #[must_use]
    pub fn inner(&self) -> &DruidDataSource {
        &self.inner
    }

    fn authorization_error() -> DruidError {
        DruidError::SqlException(Box::new(
            SqlException::new(
                0,
                Some("28000".to_owned()),
                Some("invalid authorization specification".to_owned()),
            )
            .with_class_name("java.sql.SQLInvalidAuthorizationSpecException")
            .with_assignable_type("java.sql.SQLNonTransientException"),
        ))
    }
}

impl Wrapper for DruidRdbcDataSource {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl CommonDataSource for DruidRdbcDataSource {
    fn login_timeout(&self) -> u64 {
        u64::try_from(self.inner.login_timeout()).unwrap_or(0)
    }
}

#[async_trait]
impl DataSource for DruidRdbcDataSource {
    async fn get_connection(&self) -> Result<DruidPooledConnection, DruidError> {
        self.inner.get_connection().await
    }

    async fn get_connection_with_credentials(
        &self,
        username: &str,
        password: &str,
    ) -> Result<DruidPooledConnection, DruidError> {
        match &self.credentials {
            Some((expected_username, expected_password))
                if expected_username == username && expected_password == password =>
            {
                self.get_connection().await
            }
            _ => Err(Self::authorization_error()),
        }
    }
}
