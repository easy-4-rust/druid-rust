//! Canonical SQLx database metadata actor.

use super::sqlx_connection_actor::SqlxConnectionActor;
use druid_rdbc::error::RdbcError;
use std::sync::Arc;

pub struct SqlxDatabaseMetaData {
    actor: Arc<SqlxConnectionActor>,
}

impl SqlxDatabaseMetaData {
    pub(crate) fn owned(actor: Arc<SqlxConnectionActor>) -> Self {
        Self { actor }
    }

    fn actor(&self) -> Result<&Arc<SqlxConnectionActor>, RdbcError> {
        if self.actor.is_closed() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(&self.actor)
        }
    }
}

druid_rdbc::impl_standard_wrapper!(SqlxDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl druid_rdbc::metadata::DatabaseMetaData for SqlxDatabaseMetaData {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        let actor = Arc::clone(self.actor()?);
        let product_name = actor.database_product_name().await?;
        actor.task4_trace_database_metadata();
        Ok(product_name)
    }

    async fn tables(
        &mut self,
        query: druid_rdbc::metadata::TablesQuery,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        Arc::clone(self.actor()?).tables(query).await
    }
}
