//! DuckDB 原生数据库元数据。

use druid::core::{DruidError, PhysicalDatabaseMetaData};
use duckdb::Connection;
use parking_lot::Mutex;
use std::sync::Arc;

/// DuckDB 原生连接的数据库元数据视图。
///
/// 对应 Java: `java.sql.DatabaseMetaData`。仅报告 duckdb-rs 和当前物理连接
/// 能够证明的字段，其余方法保留 trait 的明确不支持语义。
pub struct DuckDbDatabaseMetaData {
    connection: Arc<Mutex<Connection>>,
    url: String,
}

impl DuckDbDatabaseMetaData {
    /// 创建绑定当前物理连接的 DuckDB 元数据对象。
    pub(crate) fn new(connection: Arc<Mutex<Connection>>, url: impl Into<String>) -> Self {
        Self {
            connection,
            url: url.into(),
        }
    }

    async fn product_version(&self) -> Result<String, DruidError> {
        let connection = Arc::clone(&self.connection);
        tokio::task::spawn_blocking(move || {
            connection
                .lock()
                .version()
                .map_err(super::DuckDbConnectionAdapter::driver_error)
        })
        .await
        .map_err(|error| {
            DruidError::DriverError(format!("DuckDB metadata worker failed: {error}"))
        })?
    }

    async fn version_component(&self, index: usize) -> Result<i32, DruidError> {
        let version = self.product_version().await?;
        version
            .split(|character: char| !character.is_ascii_digit())
            .filter(|component| !component.is_empty())
            .nth(index)
            .and_then(|component| component.parse().ok())
            .ok_or_else(|| {
                DruidError::DriverError(format!(
                    "DuckDB version `{version}` has no numeric component {index}"
                ))
            })
    }
}

#[async_trait::async_trait]
impl PhysicalDatabaseMetaData for DuckDbDatabaseMetaData {
    async fn all_procedures_are_callable(&mut self) -> Result<bool, DruidError> {
        Ok(false)
    }

    async fn all_tables_are_selectable(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn get_url(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some(self.url.clone()))
    }

    async fn get_user_name(&mut self) -> Result<Option<String>, DruidError> {
        Ok(None)
    }

    async fn is_read_only(&mut self) -> Result<bool, DruidError> {
        Ok(false)
    }

    async fn get_database_product_name(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some("DuckDB".to_string()))
    }

    async fn get_database_product_version(&mut self) -> Result<Option<String>, DruidError> {
        self.product_version().await.map(Some)
    }

    async fn get_driver_name(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some("duckdb-rs".to_string()))
    }

    async fn get_driver_version(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some("1.10505.0".to_string()))
    }

    async fn get_driver_major_version(&mut self) -> Result<i32, DruidError> {
        self.version_component(0).await
    }

    async fn get_driver_minor_version(&mut self) -> Result<i32, DruidError> {
        self.version_component(1).await
    }

    async fn uses_local_files(&mut self) -> Result<bool, DruidError> {
        Ok(!self.url.ends_with(":memory:"))
    }

    async fn uses_local_file_per_table(&mut self) -> Result<bool, DruidError> {
        Ok(false)
    }

    async fn stores_lower_case_identifiers(&mut self) -> Result<bool, DruidError> {
        Ok(false)
    }

    async fn stores_mixed_case_identifiers(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn supports_mixed_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn get_identifier_quote_string(&mut self) -> Result<Option<String>, DruidError> {
        Ok(Some("\"".to_string()))
    }

    async fn supports_transactions(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }

    async fn supports_savepoints(&mut self) -> Result<bool, DruidError> {
        Ok(false)
    }

    async fn supports_batch_updates(&mut self) -> Result<bool, DruidError> {
        Ok(true)
    }
}
