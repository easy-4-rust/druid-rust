//! Canonical connection state captured at first pool admission.

use super::DruidError;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError, RdbcErrorKind};

/// State restored before a driver connection returns to the idle queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDefaults {
    auto_commit: bool,
    read_only: bool,
    holdability: i32,
    transaction_isolation: u8,
    catalog: Option<String>,
    schema: Option<String>,
    auto_commit_supported: bool,
    read_only_supported: bool,
    catalog_supported: bool,
    schema_supported: bool,
    warnings_supported: bool,
}

impl Default for ConnectionDefaults {
    fn default() -> Self {
        Self {
            auto_commit: true,
            read_only: false,
            holdability: 0,
            transaction_isolation: 2,
            catalog: None,
            schema: None,
            auto_commit_supported: false,
            read_only_supported: false,
            catalog_supported: false,
            schema_supported: false,
            warnings_supported: false,
        }
    }
}

impl ConnectionDefaults {
    /// Captures only state that the concrete connection truthfully supports.
    pub(crate) async fn capture_standard(
        connection: &mut dyn Connection,
    ) -> Result<Self, DruidError> {
        let auto_commit = optional_state(connection.auto_commit())?;
        let read_only = optional_state(connection.read_only())?;
        let catalog = optional_state(connection.catalog())?;
        let schema = optional_state(connection.schema())?;
        let warnings_supported = optional_state(connection.warnings().await)?.is_some();
        Ok(Self {
            auto_commit: auto_commit.unwrap_or(true),
            read_only: read_only.unwrap_or(false),
            holdability: 0,
            transaction_isolation: 2,
            catalog: catalog.clone().flatten(),
            schema: schema.clone().flatten(),
            auto_commit_supported: auto_commit.is_some(),
            read_only_supported: read_only.is_some(),
            catalog_supported: catalog.is_some(),
            schema_supported: schema.is_some(),
            warnings_supported,
        })
    }

    pub(crate) fn standard_auto_commit(&self) -> Option<bool> {
        self.auto_commit_supported.then_some(self.auto_commit)
    }

    pub(crate) fn standard_read_only(&self) -> Option<bool> {
        self.read_only_supported.then_some(self.read_only)
    }

    pub(crate) const fn standard_warnings_supported(&self) -> bool {
        self.warnings_supported
    }

    /// Returns the captured auto-commit value.
    #[must_use]
    pub const fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    /// Returns the captured read-only value.
    #[must_use]
    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    /// Returns the compatibility holdability snapshot.
    #[must_use]
    pub const fn holdability(&self) -> i32 {
        self.holdability
    }

    /// Returns the compatibility transaction-isolation snapshot.
    #[must_use]
    pub const fn transaction_isolation(&self) -> u8 {
        self.transaction_isolation
    }

    pub(crate) fn catalog(&self) -> Option<&str> {
        self.catalog.as_deref()
    }

    pub(crate) fn schema(&self) -> Option<&str> {
        self.schema.as_deref()
    }

    pub(crate) fn standard_needs_reset(
        &self,
        connection: &dyn Connection,
    ) -> Result<bool, DruidError> {
        Ok(
            (self.read_only_supported && connection.read_only()? != self.read_only)
                || (self.auto_commit_supported && connection.auto_commit()? != self.auto_commit)
                || (self.catalog_supported && connection.catalog()?.as_deref() != self.catalog())
                || (self.schema_supported && connection.schema()?.as_deref() != self.schema()),
        )
    }

    pub(crate) async fn reset_standard_state(
        &self,
        connection: &mut dyn Connection,
    ) -> Result<(), DruidError> {
        if self.read_only_supported && connection.read_only()? != self.read_only {
            connection.set_read_only(self.read_only).await?;
        }
        if self.catalog_supported && connection.catalog()?.as_deref() != self.catalog() {
            connection.set_catalog(self.catalog()).await?;
        }
        if self.schema_supported && connection.schema()?.as_deref() != self.schema() {
            connection.set_schema(self.schema()).await?;
        }
        if self.auto_commit_supported && connection.auto_commit()? != self.auto_commit {
            connection.set_auto_commit(self.auto_commit).await?;
        }
        Ok(())
    }
}

fn optional_state<T>(result: Result<T, RdbcError>) -> Result<Option<T>, DruidError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if matches!(
                error.kind(),
                RdbcErrorKind::UnsupportedOperation | RdbcErrorKind::SqlFeatureNotSupported
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}
