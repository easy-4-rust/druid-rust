use std::any::TypeId;
use std::fmt;

use async_trait::async_trait;
use druid::core::DruidPooledConnection;
use druid::pool::DruidDataSource;
use druid::sql::DataSource;
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq as _;

use crate::driver::{DriverRegistryError, DruidDatabasePoolBuilder};

/// 在 wrapper 层实现的 Druid RDBC 数据源。
///
/// 对应 Java: `javax.sql.DataSource`。底层只持有一个 canonical
/// [`DruidDataSource`]，显式凭据仅用于校验，禁止每次调用创建新连接池。
pub struct DruidRdbcDataSource {
    inner: DruidDataSource,
    credential_digest: Option<[u8; 32]>,
}

impl fmt::Debug for DruidRdbcDataSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DruidRdbcDataSource")
            .field("credentials_configured", &self.credential_digest.is_some())
            .finish_non_exhaustive()
    }
}

fn credential_digest(username: &str, password: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"druid-rdbc-data-source-credentials-v1\0");
    // 长度前缀保持用户名和密码的边界，避免分隔符移动形成同一字节串。
    digest.update((username.len() as u64).to_be_bytes());
    digest.update(username.as_bytes());
    digest.update((password.len() as u64).to_be_bytes());
    digest.update(password.as_bytes());
    digest.finalize().into()
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
            credential_digest: None,
        }
    }

    /// 包装现有数据源并声明其固定凭据。
    #[must_use]
    pub fn with_credentials(
        inner: DruidDataSource,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let username = username.into();
        let password = password.into();
        Self {
            inner,
            credential_digest: Some(credential_digest(&username, &password)),
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
        use druid_rdbc::wrapper::StandardUnwrapped;

        if target == TypeId::of::<dyn druid_rdbc::data_source::DataSource>() {
            Some(StandardUnwrapped::DataSource(self))
        } else if target == TypeId::of::<dyn druid_rdbc::data_source::CommonDataSource>() {
            Some(StandardUnwrapped::CommonDataSource(self))
        } else if target == TypeId::of::<dyn druid_rdbc::wrapper::Wrapper>() {
            Some(StandardUnwrapped::Wrapper(self))
        } else {
            None
        }
    }
}

impl druid_rdbc::data_source::CommonDataSource for DruidRdbcDataSource {
    fn try_login_timeout(&self) -> Result<Option<std::time::Duration>, RdbcError> {
        druid_rdbc::data_source::CommonDataSource::try_login_timeout(&self.inner)
    }

    fn set_login_timeout(&mut self, timeout: std::time::Duration) -> Result<(), RdbcError> {
        druid_rdbc::data_source::CommonDataSource::set_login_timeout(&mut self.inner, timeout)
    }

    fn get_log_writer(&self) -> Option<druid_rdbc::data_source::RdbcLogWriter> {
        druid_rdbc::data_source::CommonDataSource::get_log_writer(&self.inner)
    }

    fn set_log_writer(
        &self,
        writer: Option<druid_rdbc::data_source::RdbcLogWriter>,
    ) -> Result<(), RdbcError> {
        druid_rdbc::data_source::CommonDataSource::set_log_writer(&self.inner, writer)
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
        let candidate = credential_digest(username, password);
        if self
            .credential_digest
            .as_ref()
            .is_some_and(|expected| bool::from(candidate.ct_eq(expected)))
        {
            self.get_connection().await
        } else {
            let error = Self::authorization_error();
            if let Err(log_error) = druid_rdbc::data_source::CommonDataSource::write_diagnostic(
                self,
                "connection_credentials",
                &error,
            ) {
                tracing::warn!(
                    operation = "connection_credentials",
                    error_kind = ?log_error.kind(),
                    "ordinary data-source diagnostic writer failed; preserving the authorization error"
                );
            }
            Err(error)
        }
    }
}
