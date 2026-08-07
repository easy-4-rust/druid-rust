use druid::core::{DruidError, PhysicalDatabaseMetaData};

/// JDBC Agent 建立 session 时从真实 `DatabaseMetaData` 捕获的稳定快照。
#[derive(Debug, Clone)]
pub struct JdbcAgentDatabaseMetaData {
    url: String,
    driver_name: Option<String>,
    driver_version: Option<String>,
    database_product_name: Option<String>,
    database_product_version: Option<String>,
    supports_transactions: bool,
}

impl JdbcAgentDatabaseMetaData {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        url: impl Into<String>,
        driver_name: Option<String>,
        driver_version: Option<String>,
        database_product_name: Option<String>,
        database_product_version: Option<String>,
        supports_transactions: bool,
    ) -> Self {
        Self {
            url: url.into(),
            driver_name,
            driver_version,
            database_product_name,
            database_product_version,
            supports_transactions,
        }
    }
}

#[async_trait::async_trait]
impl PhysicalDatabaseMetaData for JdbcAgentDatabaseMetaData {
    async fn all_tables_are_selectable(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }
    async fn get_url(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some(self.url.clone()))
    }
    async fn is_read_only(&mut self) -> Result<bool, DruidError> {
        Ok(false)
    }
    async fn get_database_product_name(&mut self) -> Result<Option<String>, DruidError> {
        Ok(self.database_product_name.clone())
    }
    async fn get_database_product_version(&mut self) -> Result<Option<String>, DruidError> {
        Ok(self.database_product_version.clone())
    }
    async fn get_driver_name(&mut self) -> Result<Option<String>, DruidError> {
        Ok(self.driver_name.clone())
    }
    async fn get_driver_version(&mut self) -> Result<Option<String>, DruidError> {
        Ok(self.driver_version.clone())
    }
    async fn supports_transactions(&mut self) -> Result<bool, DruidError> {
        Ok(self.supports_transactions)
    }
    async fn supports_batch_updates(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }
}
