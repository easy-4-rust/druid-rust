//! Wrapper-private live DatabaseMetaData bound to an owned family executor.

use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{DatabaseMetaData, DatabaseMetaDataOperation, DatabaseMetaDataRequest};
use std::sync::Arc;

/// Checks the live executor before forwarding supported family metadata operations.
pub(crate) struct StandardAdapterDatabaseMetaData {
    executor: Arc<dyn StandardExecutor>,
    product_name: Option<String>,
    driver_name: Option<String>,
    absent_operation: &'static str,
}

impl StandardAdapterDatabaseMetaData {
    pub(crate) fn new(
        executor: Arc<dyn StandardExecutor>,
        product_name: Option<String>,
        absent_operation: &'static str,
    ) -> Self {
        Self {
            executor,
            product_name,
            driver_name: None,
            absent_operation,
        }
    }

    #[cfg(feature = "jdbc-agent")]
    pub(crate) fn with_driver_name(mut self, driver_name: String) -> Self {
        self.driver_name = Some(driver_name);
        self
    }

    fn ensure_open_core(&self) -> Result<(), RdbcError> {
        self.executor.ensure_open()
    }
}

druid_rdbc::impl_standard_wrapper!(StandardAdapterDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl DatabaseMetaData for StandardAdapterDatabaseMetaData {
    async fn metadata_string(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Option<String>, RdbcError> {
        self.ensure_open_core()?;
        match request.operation() {
            DatabaseMetaDataOperation::GetDriverName => Ok(self.driver_name.clone()),
            DatabaseMetaDataOperation::GetDatabaseProductName => Ok(self.product_name.clone()),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "database_metadata_string",
            }),
        }
    }

    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        self.ensure_open_core()?;
        let product_name = self
            .product_name
            .clone()
            .ok_or(RdbcError::UnsupportedOperation {
                operation: self.absent_operation,
            });
        self.executor.task4_trace_database_metadata();
        product_name
    }

    async fn tables(
        &mut self,
        _query: druid_rdbc::metadata::TablesQuery,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.ensure_open_core()?;
        Err(RdbcError::UnsupportedOperation {
            operation: "standard_adapter_database_metadata_tables",
        })
    }
}
