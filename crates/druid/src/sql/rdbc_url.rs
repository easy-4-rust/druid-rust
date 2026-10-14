use crate::core::DruidError;
use druid_rdbc::error::RdbcError;
use druid_rdbc::url::RdbcConnectionUrl;
use std::collections::HashMap;

/// Druid's compatibility facade for the standard RDBC product connection URL.
///
/// The parser implementation lives in `druid-rdbc::url::RdbcConnectionUrl`;
/// this facade preserves the existing public Druid path and its `DruidError` result type.
#[derive(Clone, PartialEq, Eq)]
pub struct RdbcUrl {
    inner: RdbcConnectionUrl,
}

impl RdbcUrl {
    /// Parses and validates a unified RDBC URL; invalid input returns the existing Druid error.
    pub fn parse(value: &str) -> Result<Self, DruidError> {
        RdbcConnectionUrl::parse(value)
            .map(|inner| Self { inner })
            .map_err(to_druid_error)
    }

    /// Returns the original, unredacted RDBC URL; the value may contain secrets.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.inner.as_str()
    }

    /// Returns the catalog profile ID.
    #[must_use]
    pub fn profile(&self) -> &str {
        self.inner.profile()
    }

    /// Returns the endpoint, usually `host[:port]` or the first embedded path segment.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        self.inner.endpoint()
    }

    /// Returns the database, service, or file path.
    #[must_use]
    pub fn database(&self) -> &str {
        self.inner.database()
    }

    /// Returns URL-decoded connection properties; values must not be logged.
    #[must_use]
    pub fn properties(&self) -> &HashMap<String, String> {
        self.inner.properties()
    }

    /// Returns a decoded property by name, or `None` when absent.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&str> {
        self.inner.property(name)
    }

    /// Returns an observable URL without query properties so diagnostics cannot leak secrets.
    #[must_use]
    pub fn redacted(&self) -> String {
        self.inner.redacted()
    }

    /// Builds a credential-free driver URL using the native network scheme.
    pub fn network_url(&self, scheme: &str) -> Result<String, DruidError> {
        self.inner.network_url(scheme).map_err(to_druid_error)
    }

    /// Builds a credential-bearing URL only for physical driver use; do not log the result.
    pub fn authenticated_network_url(&self, scheme: &str) -> Result<String, DruidError> {
        self.inner
            .authenticated_network_url(scheme)
            .map_err(to_druid_error)
    }
}

impl std::fmt::Debug for RdbcUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RdbcUrl")
            .field("url", &self.redacted())
            .field("property_names", &self.properties().keys())
            .finish_non_exhaustive()
    }
}

fn to_druid_error(error: RdbcError) -> DruidError {
    match error {
        RdbcError::InvalidArgument(message) => DruidError::InvalidArgument(message),
        other => DruidError::Rdbc(other),
    }
}
