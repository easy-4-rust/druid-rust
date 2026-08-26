use druid_rdbc::error::RdbcError;

/// JDBC Agent 建立 session 时从真实 `DatabaseMetaData` 捕获的稳定快照。
#[derive(Debug, Clone)]
pub struct JdbcAgentDatabaseMetaData {
    database_product_name: Option<String>,
}

impl JdbcAgentDatabaseMetaData {
    pub(crate) fn new(database_product_name: Option<String>) -> Self {
        Self {
            database_product_name,
        }
    }

    fn database_product_name_core(&self) -> Option<String> {
        self.database_product_name.clone()
    }
}

druid_rdbc::impl_standard_wrapper!(JdbcAgentDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl druid_rdbc::metadata::DatabaseMetaData for JdbcAgentDatabaseMetaData {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        self.database_product_name_core()
            .ok_or(RdbcError::UnsupportedOperation {
                operation: "jdbc_agent_database_metadata_product_name_absent",
            })
    }
}
