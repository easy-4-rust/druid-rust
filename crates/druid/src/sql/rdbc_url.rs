use std::collections::HashMap;

use crate::core::DruidError;

/// Druid's unified database connection URL.
///
/// The form is `rdbc://<profile>/<endpoint>/<database>?key=value`, for example
/// `rdbc://postgresql/localhost:5432/app?user=druid`. `profile` selects a catalog entry;
/// the wrapper registry converts endpoint and database to a native driver URL. Credentials
/// belong in query properties. User-info and fragments are forbidden. Logs use `redacted`.
#[derive(Clone, PartialEq, Eq)]
pub struct RdbcUrl {
    raw: String,
    profile: String,
    endpoint: String,
    database: String,
    properties: HashMap<String, String>,
}

impl RdbcUrl {
    /// Parses and validates a unified RDBC URL.
    ///
    /// `value` must use the `rdbc` scheme and include a profile. Query names and values are URL
    /// decoded, with the last duplicate winning. Invalid syntax, user-info, or a fragment returns
    /// `InvalidArgument`.
    pub fn parse(value: &str) -> Result<Self, DruidError> {
        let parsed = url::Url::parse(value)
            .map_err(|error| DruidError::InvalidArgument(format!("invalid RDBC URL: {error}")))?;
        if parsed.scheme() != "rdbc" {
            return Err(DruidError::InvalidArgument(
                "RDBC URL scheme must be 'rdbc'".to_owned(),
            ));
        }
        if parsed.username() != "" || parsed.password().is_some() || parsed.fragment().is_some() {
            return Err(DruidError::InvalidArgument(
                "RDBC URL forbids user-info and fragments; use query properties".to_owned(),
            ));
        }
        let profile = parsed
            .host_str()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                DruidError::InvalidArgument("RDBC URL must contain a database profile".to_owned())
            })?;
        let mut segments = parsed
            .path_segments()
            .ok_or_else(|| DruidError::InvalidArgument("RDBC URL path is invalid".to_owned()))?
            .filter(|value| !value.is_empty());
        let endpoint = segments.next().unwrap_or_default().to_owned();
        let database = segments.collect::<Vec<_>>().join("/");
        let properties = parsed
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        Ok(Self {
            raw: value.to_owned(),
            profile: profile.to_owned(),
            endpoint,
            database,
            properties,
        })
    }

    /// Returns the original, unredacted RDBC URL.
    ///
    /// This value may contain secrets. Use it only for driver resolution, never diagnostics.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }
    /// Returns the catalog profile ID.
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }
    /// Returns the endpoint, usually `host[:port]` or the first embedded path segment.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    /// Returns the database, service, or file path.
    #[must_use]
    pub fn database(&self) -> &str {
        &self.database
    }
    /// Returns URL-decoded connection properties; values must not be logged.
    #[must_use]
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }
    /// Returns a decoded property value, or `None` when absent.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }

    /// Returns an observable URL without query properties so diagnostics cannot leak secrets.
    #[must_use]
    pub fn redacted(&self) -> String {
        let suffix = if self.database.is_empty() {
            String::new()
        } else {
            format!("/{}", self.database)
        };
        format!("rdbc://{}/{}{suffix}", self.profile, self.endpoint)
    }

    /// Builds a credential-free driver URL using the native network `scheme`.
    ///
    /// An empty endpoint returns `InvalidArgument`; the parsed database hierarchy is preserved.
    pub fn network_url(&self, scheme: &str) -> Result<String, DruidError> {
        if self.endpoint.is_empty() {
            return Err(DruidError::InvalidArgument(format!(
                "RDBC profile '{}' requires endpoint",
                self.profile
            )));
        }
        let suffix = if self.database.is_empty() {
            String::new()
        } else {
            format!("/{}", self.database)
        };
        Ok(format!("{scheme}://{}{suffix}", self.endpoint))
    }

    /// Builds a native URL and encodes `user` and `password` as user-info.
    ///
    /// The returned URL may contain credentials and is only for the physical driver. Invalid
    /// endpoints, target URLs, or credentials return `InvalidArgument`.
    pub fn authenticated_network_url(&self, scheme: &str) -> Result<String, DruidError> {
        let mut parsed = url::Url::parse(&self.network_url(scheme)?)
            .map_err(|error| DruidError::InvalidArgument(format!("invalid target URL: {error}")))?;
        if let Some(username) = self.property("user") {
            parsed
                .set_username(username)
                .map_err(|()| DruidError::InvalidArgument("invalid RDBC username".to_owned()))?;
        }
        if let Some(password) = self.property("password") {
            parsed
                .set_password(Some(password))
                .map_err(|()| DruidError::InvalidArgument("invalid RDBC password".to_owned()))?;
        }
        Ok(parsed.to_string())
    }
}

impl std::fmt::Debug for RdbcUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RdbcUrl")
            .field("url", &self.redacted())
            .field("property_names", &self.properties.keys())
            .finish_non_exhaustive()
    }
}
