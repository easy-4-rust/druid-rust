use druid_rdbc::error::RdbcError;
use druid_rdbc::spi::PhysicalDatabaseMetaData;

/// Turso/libSQL 连接公开的可证明数据库元数据。
pub struct LibSqlDatabaseMetaData {
    url: String,
}

impl LibSqlDatabaseMetaData {
    pub(crate) fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

#[async_trait::async_trait]
impl PhysicalDatabaseMetaData for LibSqlDatabaseMetaData {
    async fn all_tables_are_selectable(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }
    async fn get_url(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some(self.url.clone()))
    }
    async fn is_read_only(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    async fn get_database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some("Turso/libSQL".to_owned()))
    }
    async fn get_driver_name(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some("libsql-rs".to_owned()))
    }
    async fn get_driver_version(&mut self) -> Result<Option<String>, RdbcError> {
        Ok(Some("0.9.30".to_owned()))
    }
    async fn supports_transactions(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }
    async fn supports_batch_updates(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }
}
