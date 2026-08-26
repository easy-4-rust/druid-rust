//! Canonical SQLx database metadata actor.

mod sqlite_constants;
mod sqlite_queries;

use super::sqlx_connection_actor::SqlxConnectionActor;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    DatabaseMetaDataArgument, DatabaseMetaDataOperation, DatabaseMetaDataRequest, RowIdLifetime,
};
use druid_rdbc::result_set::ResultSet;
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

    async fn database_version_component(&self, index: usize) -> Result<i32, RdbcError> {
        let version = self.actor()?.database_product_version().await?;
        version
            .split(|character: char| !character.is_ascii_digit())
            .filter(|component| !component.is_empty())
            .nth(index)
            .and_then(|component| component.parse::<i32>().ok())
            .ok_or_else(|| {
                RdbcError::DriverError(format!(
                    "database version {version:?} has no numeric component {index}"
                ))
            })
    }

    fn integer_argument(request: &DatabaseMetaDataRequest, index: usize) -> Result<i32, RdbcError> {
        match request.arguments().get(index) {
            Some(DatabaseMetaDataArgument::Integer(value)) => Ok(*value),
            _ => Err(RdbcError::InvalidArgument(format!(
                "metadata argument {index} is not Integer"
            ))),
        }
    }
}

druid_rdbc::impl_standard_wrapper!(SqlxDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl druid_rdbc::metadata::DatabaseMetaData for SqlxDatabaseMetaData {
    async fn metadata_boolean(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<bool, RdbcError> {
        let operation = request.operation();
        match operation {
            DatabaseMetaDataOperation::IsReadOnly => self.actor()?.database_read_only().await,
            DatabaseMetaDataOperation::SupportsTransactions
            | DatabaseMetaDataOperation::SupportsBatchUpdates
            | DatabaseMetaDataOperation::SupportsSavepoints => Ok(true),
            DatabaseMetaDataOperation::SupportsNamedParameters => {
                Ok(self.actor()?.backend_key() == "sqlite")
            }
            DatabaseMetaDataOperation::SupportsGetGeneratedKeys => Ok(std::matches!(
                self.actor()?.backend_key(),
                "sqlite" | "mysql"
            )),
            DatabaseMetaDataOperation::SupportsTransactionIsolationLevel => {
                let level = Self::integer_argument(&request, 0)?;
                Ok(if self.actor()?.backend_key() == "sqlite" {
                    level == 8
                } else {
                    level == 2
                })
            }
            DatabaseMetaDataOperation::SupportsResultSetType => Ok(std::matches!(
                Self::integer_argument(&request, 0)?,
                1003 | 1004
            )),
            DatabaseMetaDataOperation::SupportsResultSetConcurrency => Ok(std::matches!(
                Self::integer_argument(&request, 0)?,
                1003 | 1004
            )
                && Self::integer_argument(&request, 1)? == 1007),
            DatabaseMetaDataOperation::SupportsResultSetHoldability => {
                Ok(Self::integer_argument(&request, 0)? == 2)
            }
            DatabaseMetaDataOperation::SupportsFullOuterJoins
                if self.actor()?.backend_key() == "sqlite" =>
            {
                let major = self.database_version_component(0).await?;
                let minor = self.database_version_component(1).await?;
                Ok(major > 3 || (major == 3 && minor >= 39))
            }
            DatabaseMetaDataOperation::GeneratedKeyAlwaysReturned => Ok(false),
            _ if self.actor()?.backend_key() == "sqlite" => sqlite_constants::boolean(operation)
                .ok_or(RdbcError::UnsupportedOperation {
                    operation: operation.as_str(),
                }),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: operation.as_str(),
            }),
        }
    }

    async fn metadata_string(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Option<String>, RdbcError> {
        let operation = request.operation();
        match operation {
            DatabaseMetaDataOperation::GetUrl => Ok(Some(self.actor()?.database_url().to_owned())),
            DatabaseMetaDataOperation::GetUserName => self.actor()?.database_user_name().await,
            DatabaseMetaDataOperation::GetDatabaseProductName => {
                self.actor()?.database_product_name().await.map(Some)
            }
            DatabaseMetaDataOperation::GetDatabaseProductVersion => {
                self.actor()?.database_product_version().await.map(Some)
            }
            DatabaseMetaDataOperation::GetDriverName => Ok(Some("sqlx".to_owned())),
            DatabaseMetaDataOperation::GetDriverVersion => {
                Ok(Some(env!("CARGO_PKG_VERSION").to_owned()))
            }
            DatabaseMetaDataOperation::GetIdentifierQuoteString => Ok(Some(
                if self.actor()?.backend_key() == "mysql" {
                    "`"
                } else {
                    "\""
                }
                .to_owned(),
            )),
            DatabaseMetaDataOperation::GetSearchStringEscape => Ok(Some("\\".to_owned())),
            DatabaseMetaDataOperation::GetCatalogSeparator => Ok(Some(".".to_owned())),
            DatabaseMetaDataOperation::GetSqlKeywords if self.actor()?.backend_key() == "sqlite" => {
                Ok(Some("ABORT,ACTION,AFTER,ANALYZE,ATTACH,AUTOINCREMENT,BEFORE,CASCADE,CONFLICT,DATABASE,DEFERRABLE,DEFERRED,DESC,DETACH,EXCLUSIVE,EXPLAIN,FAIL,GLOB,IGNORE,INDEX,INDEXED,INITIALLY,INSTEAD,ISNULL,KEY,LIMIT,NOTNULL,OFFSET,PLAN,PRAGMA,QUERY,RAISE,REGEXP,REINDEX,RENAME,REPLACE,RESTRICT,TEMP,TEMPORARY,TRANSACTION,VACUUM,VIEW,VIRTUAL".to_owned()))
            }
            DatabaseMetaDataOperation::GetNumericFunctions
            | DatabaseMetaDataOperation::GetStringFunctions
            | DatabaseMetaDataOperation::GetSystemFunctions
            | DatabaseMetaDataOperation::GetExtraNameCharacters
                if self.actor()?.backend_key() == "sqlite" => Ok(Some(String::new())),
            DatabaseMetaDataOperation::GetTimeDateFunctions
                if self.actor()?.backend_key() == "sqlite" =>
            {
                Ok(Some("DATE,TIME,DATETIME,JULIANDAY,STRFTIME".to_owned()))
            }
            DatabaseMetaDataOperation::GetSchemaTerm if self.actor()?.backend_key() == "sqlite" => {
                Ok(Some("schema".to_owned()))
            }
            DatabaseMetaDataOperation::GetProcedureTerm
                if self.actor()?.backend_key() == "sqlite" =>
            {
                Ok(Some("not_implemented".to_owned()))
            }
            DatabaseMetaDataOperation::GetCatalogTerm if self.actor()?.backend_key() == "sqlite" => {
                Ok(Some("catalog".to_owned()))
            }
            _ => Err(RdbcError::UnsupportedOperation {
                operation: operation.as_str(),
            }),
        }
    }

    async fn metadata_integer(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<i32, RdbcError> {
        let operation = request.operation();
        match operation {
            DatabaseMetaDataOperation::GetDatabaseMajorVersion => {
                self.database_version_component(0).await
            }
            DatabaseMetaDataOperation::GetDatabaseMinorVersion => {
                self.database_version_component(1).await
            }
            DatabaseMetaDataOperation::GetRdbcMajorVersion => Ok(4),
            DatabaseMetaDataOperation::GetRdbcMinorVersion => Ok(2),
            DatabaseMetaDataOperation::GetMaxConnections => Ok(0),
            _ if self.actor()?.backend_key() == "sqlite" => sqlite_constants::integer(operation)
                .ok_or(RdbcError::UnsupportedOperation {
                    operation: operation.as_str(),
                }),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: operation.as_str(),
            }),
        }
    }

    async fn metadata_result_set(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set_core(request).await
    }

    async fn metadata_row_id_lifetime(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<RowIdLifetime, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: request.operation().as_str(),
        })
    }

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
