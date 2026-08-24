use super::HttpSqlProvider;
use druid_rdbc::error::RdbcError;
use druid_rdbc::spi::PhysicalDatabaseMetaData;

/// HTTP SQL 连接公开的可证明数据库元数据。
pub struct HttpSqlDatabaseMetaData {
    provider: HttpSqlProvider,
    endpoint: String,
    product_version: Option<String>,
}

impl HttpSqlDatabaseMetaData {
    pub(crate) fn new(
        provider: HttpSqlProvider,
        endpoint: impl Into<String>,
        product_version: Option<String>,
    ) -> Self {
        Self {
            provider,
            endpoint: endpoint.into(),
            product_version,
        }
    }
}

#[async_trait::async_trait]
impl PhysicalDatabaseMetaData for HttpSqlDatabaseMetaData {
    async fn all_tables_are_selectable(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }

    async fn get_url(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(self.endpoint.clone()))
    }

    async fn is_read_only(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    async fn get_database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(match self.provider {
            HttpSqlProvider::Rqlite => "rqlite".to_owned(),
            HttpSqlProvider::CloudflareD1 => "Cloudflare D1".to_owned(),
        }))
    }

    async fn get_database_product_version(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(self.product_version.clone())
    }

    async fn get_driver_name(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(format!("druid-rust/{}", self.provider.as_str())))
    }

    async fn get_driver_version(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(env!("CARGO_PKG_VERSION").to_owned()))
    }

    async fn supports_transactions(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    async fn supports_batch_updates(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }
}
