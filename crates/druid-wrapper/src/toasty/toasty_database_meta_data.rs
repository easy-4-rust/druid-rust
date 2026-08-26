//! Canonical Toasty database metadata.

use druid_rdbc::error::RdbcError;

pub struct ToastyDatabaseMetaData {
    driver_name: String,
}

impl ToastyDatabaseMetaData {
    pub(crate) fn owned(_url: String, driver_name: String, _read_only: bool) -> Self {
        Self { driver_name }
    }

    fn product_name_core(&self) -> Option<&'static str> {
        if self.driver_name.eq_ignore_ascii_case("sqlite")
            || self.driver_name.eq_ignore_ascii_case("turso")
        {
            Some("SQLite")
        } else if self.driver_name.eq_ignore_ascii_case("mysql") {
            Some("MySQL")
        } else if self.driver_name.eq_ignore_ascii_case("postgresql")
            || self.driver_name.eq_ignore_ascii_case("postgres")
        {
            Some("PostgreSQL")
        } else {
            None
        }
    }
}

druid_rdbc::impl_standard_wrapper!(ToastyDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl druid_rdbc::metadata::DatabaseMetaData for ToastyDatabaseMetaData {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        self.product_name_core()
            .map(str::to_owned)
            .ok_or(RdbcError::UnsupportedOperation {
                operation: "toasty_database_metadata_product_name",
            })
    }
}
