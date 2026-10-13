use std::any::TypeId;

use async_trait::async_trait;
use druid::core::DruidPooledConnection;
use druid::pool::DruidDataSource;
use druid::sql::{CommonDataSource, DataSource};
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;

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
    /// 从统一 RDBC URL 构建目录驱动的 canonical Druid `DataSource`。
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

    fn authorization_error() -> RdbcError {
        RdbcError::SqlException(Box::new(
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

impl druid_rdbc::wrapper::Wrapper for DruidRdbcDataSource {
    fn wrapper_view(&self) -> &dyn druid_rdbc::wrapper::Wrapper {
        self
    }

    fn canonical_unwrap(
        &self,
        target: TypeId,
    ) -> Option<druid_rdbc::wrapper::StandardUnwrapped<'_>> {
        (target == TypeId::of::<dyn druid_rdbc::wrapper::Wrapper>())
            .then_some(druid_rdbc::wrapper::StandardUnwrapped::Wrapper(self))
    }
}

impl CommonDataSource for DruidRdbcDataSource {
    fn login_timeout(&self) -> u64 {
        u64::try_from(self.inner.login_timeout()).unwrap_or(0)
    }
}

impl druid_rdbc::data_source::CommonDataSource for DruidRdbcDataSource {
    fn get_log_writer(&self) -> Option<druid_rdbc::data_source::RdbcLogWriter> {
        druid::sql::CommonDataSource::get_log_writer(&self.inner)
    }

    fn set_log_writer(
        &self,
        writer: Option<druid_rdbc::data_source::RdbcLogWriter>,
    ) -> Result<(), RdbcError> {
        druid::sql::CommonDataSource::set_log_writer(&self.inner, writer)
    }

    fn get_parent_logger(&self) -> Result<&'static str, RdbcError> {
        druid_rdbc::data_source::CommonDataSource::get_parent_logger(&self.inner)
    }
}

#[async_trait]
impl druid_rdbc::data_source::DataSource for DruidRdbcDataSource {
    async fn connection(&self) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        let connection = self.inner.get_connection().await.map_err(RdbcError::from)?;
        Ok(Box::new(connection))
    }

    async fn connection_with_credentials(
        &self,
        user: &str,
        password: &str,
    ) -> Result<Box<dyn druid_rdbc::connection::Connection>, RdbcError> {
        let connection = self.get_connection_with_credentials(user, password).await?;
        Ok(Box::new(connection))
    }
}

#[async_trait]
impl DataSource for DruidRdbcDataSource {
    async fn get_connection(&self) -> Result<DruidPooledConnection, RdbcError> {
        self.inner.get_connection().await.map_err(RdbcError::from)
    }

    async fn get_connection_with_credentials(
        &self,
        username: &str,
        password: &str,
    ) -> Result<DruidPooledConnection, RdbcError> {
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
