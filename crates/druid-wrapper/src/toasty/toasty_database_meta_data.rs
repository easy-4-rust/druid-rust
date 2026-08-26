//! Canonical Toasty database metadata.

use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    DatabaseMetaDataArgument, DatabaseMetaDataOperation, DatabaseMetaDataRequest,
};

pub struct ToastyDatabaseMetaData {
    url: String,
    driver_name: String,
    read_only: bool,
}

impl ToastyDatabaseMetaData {
    pub(crate) fn owned(url: String, driver_name: String, read_only: bool) -> Self {
        Self {
            url,
            driver_name,
            read_only,
        }
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
    async fn metadata_string(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Option<String>, RdbcError> {
        match request.operation() {
            DatabaseMetaDataOperation::GetUrl => Ok(Some(self.url.clone())),
            DatabaseMetaDataOperation::GetUserName
            | DatabaseMetaDataOperation::GetDriverVersion => Ok(None),
            DatabaseMetaDataOperation::GetDatabaseProductName => {
                self.product_name_core().map(str::to_owned).map(Some).ok_or(
                    RdbcError::UnsupportedOperation {
                        operation: "toasty_database_metadata_product_name",
                    },
                )
            }
            DatabaseMetaDataOperation::GetDriverName => Ok(Some(self.driver_name.clone())),
            DatabaseMetaDataOperation::GetIdentifierQuoteString => Ok(Some(
                if self.driver_name.eq_ignore_ascii_case("mysql") {
                    "`"
                } else {
                    "\""
                }
                .to_owned(),
            )),
            DatabaseMetaDataOperation::GetSearchStringEscape => Ok(Some("\\".to_owned())),
            DatabaseMetaDataOperation::GetCatalogSeparator => Ok(Some(".".to_owned())),
            operation => Err(RdbcError::UnsupportedOperation {
                operation: operation.as_str(),
            }),
        }
    }

    async fn metadata_boolean(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<bool, RdbcError> {
        match request.operation() {
            DatabaseMetaDataOperation::IsReadOnly => Ok(self.read_only),
            DatabaseMetaDataOperation::SupportsTransactions
            | DatabaseMetaDataOperation::SupportsBatchUpdates
            | DatabaseMetaDataOperation::SupportsSavepoints => Ok(true),
            DatabaseMetaDataOperation::SupportsNamedParameters => Ok(false),
            DatabaseMetaDataOperation::SupportsTransactionIsolationLevel => {
                let level = match request.arguments().first() {
                    Some(DatabaseMetaDataArgument::Integer(level)) => *level,
                    _ => {
                        return Err(RdbcError::InvalidArgument(
                            "metadata isolation level is missing".to_owned(),
                        ));
                    }
                };
                Ok(if self.product_name_core() == Some("SQLite") {
                    level == 8
                } else {
                    level == 2
                })
            }
            operation => Err(RdbcError::UnsupportedOperation {
                operation: operation.as_str(),
            }),
        }
    }

    async fn metadata_integer(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<i32, RdbcError> {
        match request.operation() {
            DatabaseMetaDataOperation::GetMaxConnections => Ok(0),
            operation => Err(RdbcError::UnsupportedOperation {
                operation: operation.as_str(),
            }),
        }
    }

    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        self.product_name_core()
            .map(str::to_owned)
            .ok_or(RdbcError::UnsupportedOperation {
                operation: "toasty_database_metadata_product_name",
            })
    }
}
