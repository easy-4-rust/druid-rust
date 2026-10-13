use super::CommonDataSource;
use crate::core::{DruidPooledConnection, RdbcError};

/// Factory for connections to the physical data source represented by this object.
///
/// Corresponds to Java: `javax.sql.DataSource`. It is the preferred server-side connection
/// entry point and may participate transparently in pooling infrastructure. Implementations
/// return Druid logical pooled connections and never create a second pool per call.
#[async_trait::async_trait]
pub trait DataSource: CommonDataSource + Sync {
    /// Obtains a connection from this data source.
    ///
    /// Returns a usable Druid logical connection. Timeout, closed/disabled source, and driver
    /// failures retain their SQL exception semantics. Corresponds to Java: `getConnection()`.
    async fn get_connection(&self) -> Result<DruidPooledConnection, RdbcError>;

    /// Obtains a connection with an explicit user name and password.
    ///
    /// Credentials apply only to this request and must never be logged. Unsupported per-call
    /// credentials and authentication failures return database access errors. Corresponds to
    /// Java: `DataSource#getConnection(String, String)`.
    async fn get_connection_with_credentials(
        &self,
        _username: &str,
        _password: &str,
    ) -> Result<DruidPooledConnection, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "data_source_credentials",
        })
    }
}

impl CommonDataSource for crate::pool::DruidDataSource {
    fn login_timeout(&self) -> u64 {
        u64::try_from(crate::pool::DruidDataSource::login_timeout(self)).unwrap_or(0)
    }

    fn get_log_writer(&self) -> Option<super::RdbcLogWriter> {
        self.log_writer.read().clone()
    }

    fn set_log_writer(&self, writer: Option<super::RdbcLogWriter>) -> Result<(), RdbcError> {
        *self.log_writer.write() = writer;
        Ok(())
    }
}

impl druid_rdbc::data_source::CommonDataSource for crate::pool::DruidDataSource {
    fn get_log_writer(&self) -> Option<druid_rdbc::data_source::RdbcLogWriter> {
        self.log_writer.read().clone()
    }

    fn set_log_writer(
        &self,
        writer: Option<druid_rdbc::data_source::RdbcLogWriter>,
    ) -> Result<(), RdbcError> {
        *self.log_writer.write() = writer;
        Ok(())
    }

    fn get_parent_logger(&self) -> Result<&'static str, RdbcError> {
        Ok("druid.pool")
    }
}

#[async_trait::async_trait]
impl DataSource for crate::pool::DruidDataSource {
    async fn get_connection(&self) -> Result<DruidPooledConnection, RdbcError> {
        crate::pool::DruidDataSource::get_connection(self)
            .await
            .map_err(RdbcError::from)
    }
}

impl CommonDataSource for crate::dynamic::DynamicDataSource {}

#[async_trait::async_trait]
impl DataSource for crate::dynamic::DynamicDataSource {
    async fn get_connection(&self) -> Result<DruidPooledConnection, RdbcError> {
        crate::dynamic::DynamicDataSource::get_connection(self)
            .await
            .map_err(RdbcError::from)
    }
}
