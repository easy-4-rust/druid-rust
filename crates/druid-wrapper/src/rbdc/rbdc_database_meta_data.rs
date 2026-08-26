//! Canonical RBDC database metadata.

use druid_rdbc::error::RdbcError;

pub struct RbdcDatabaseMetaData {
    driver_name: String,
}

impl RbdcDatabaseMetaData {
    pub(crate) fn owned(driver_name: String) -> Self {
        Self { driver_name }
    }

    fn product_name_core(&self) -> Option<&'static str> {
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

druid_rdbc::impl_standard_wrapper!(RbdcDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl druid_rdbc::metadata::DatabaseMetaData for RbdcDatabaseMetaData {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        self.product_name_core()
            .map(str::to_owned)
            .ok_or(RdbcError::UnsupportedOperation {
                operation: "rbdc_database_metadata_product_name",
            })
    }
}
