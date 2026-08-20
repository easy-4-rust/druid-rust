//! RBDC 数据库元数据 Adapter。

use druid_core::core::{DruidError, PhysicalDatabaseMetaData};

/// RBDC driver identity 的数据库元数据视图。
///
/// RBDC 4.9 Connection SPI 不提供 JDBC metadata；本对象只报告连接 Adapter
/// 已证明的能力，其他方法保持明确 unsupported。
pub struct RbdcDatabaseMetaData<'connection> {
    driver_name: &'connection str,
}

impl<'connection> RbdcDatabaseMetaData<'connection> {
    /// 创建 RBDC metadata。
    pub(crate) fn new(driver_name: &'connection str) -> Self {
        Self { driver_name }
    }

    fn is_mysql(&self) -> bool {
        self.driver_name.to_ascii_lowercase().contains("mysql")
    }

    fn product_name(&self) -> Option<&'static str> {
        let driver = self.driver_name.to_ascii_lowercase();
        if driver.contains("sqlite") {
            Some("SQLite")
        } else if driver.contains("mysql") || driver.contains("mariadb") {
            Some("MySQL")
        } else if driver.contains("postgres") {
            Some("PostgreSQL")
        } else if driver.contains("mssql") || driver.contains("sqlserver") {
            Some("Microsoft SQL Server")
        } else {
            None
        }
    }
}

#[async_trait::async_trait]
impl PhysicalDatabaseMetaData for RbdcDatabaseMetaData<'_> {
    async fn get_url(&mut self) -> Result<Option<String>, DruidError> {
        // RBDC Connection 在创建后不公开原始 URL。
        Ok(None)
    }

    async fn get_user_name(&mut self) -> Result<Option<String>, DruidError> {
        Ok(None)
    }

    async fn get_database_product_name(&mut self) -> Result<Option<String>, DruidError> {
        self.product_name()
            .map(str::to_owned)
            .map(Some)
            .ok_or(DruidError::UnsupportedOperation {
                operation: "rbdc_database_metadata_product_name",
            })
    }

    async fn get_driver_name(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some(self.driver_name.to_owned()))
    }

    async fn get_driver_version(&mut self) -> Result<Option<String>, DruidError> {
        Ok(None)
    }

    async fn get_identifier_quote_string(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some(if self.is_mysql() { "`" } else { "\"" }.to_owned()))
    }

    async fn get_search_string_escape(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some("\\".to_owned()))
    }

    async fn get_catalog_separator(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some(".".to_owned()))
    }

    async fn get_max_connections(&mut self) -> Result<i32, DruidError> {
        Ok(0)
    }

    async fn supports_transactions(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn supports_batch_updates(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn supports_savepoints(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn supports_named_parameters(&mut self) -> Result<bool, DruidError> {
        Ok(false)
    }
}
