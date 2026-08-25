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

    fn database_product_name_core() -> String {
        "Turso/libSQL".to_owned()
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
        Ok(Some(Self::database_product_name_core()))
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

druid_rdbc::impl_standard_wrapper!(LibSqlDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl druid_rdbc::metadata::DatabaseMetaData for LibSqlDatabaseMetaData {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        Ok(Self::database_product_name_core())
    }
}
