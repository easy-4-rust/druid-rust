//! Wrapper-private live DatabaseMetaData bound to an owned family executor.

use crate::standard_adapter_executor::StandardExecutor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::DatabaseMetaData;
use std::sync::Arc;

/// Checks the live executor before forwarding supported family metadata operations.
pub(crate) struct StandardAdapterDatabaseMetaData {
    executor: Arc<dyn StandardExecutor>,
    product_name: Option<String>,
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
            absent_operation,
        }
    }

    fn ensure_open_core(&self) -> Result<(), RdbcError> {
        self.executor.ensure_open()
    }
}

druid_rdbc::impl_standard_wrapper!(StandardAdapterDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl DatabaseMetaData for StandardAdapterDatabaseMetaData {
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
