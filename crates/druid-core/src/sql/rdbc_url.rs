use std::collections::HashMap;

use crate::core::DruidError;

/// Druid's unified database connection URL.
///
/// The canonical form follows Java's JDBC subprotocol convention:
/// `rdbc:<profile>://<endpoint>/<database>?key=value`, for example
/// `rdbc:postgresql://localhost:5432/app?sslmode=require`. The legacy
/// `rdbc://<profile>/<endpoint>/<database>` form remains accepted for compatibility.
/// `profile` selects a catalog entry; the wrapper registry converts the endpoint, database,
/// and properties to a native driver URL. Credentials belong in connection properties.
/// User-info and fragments are forbidden. Logs use `redacted`.
#[derive(Clone, PartialEq, Eq)]
pub struct RdbcUrl {
    raw: String,
    profile: String,
    endpoint: String,
    database: String,
    properties: HashMap<String, String>,
    subprotocol_style: bool,
}

impl RdbcUrl {
    /// Parses and validates a unified RDBC URL.
    ///
    /// `value` must use the `rdbc` scheme and include a profile. Query names and values are URL
    /// decoded, with the last duplicate winning. Invalid syntax, user-info, or a fragment returns
    /// `InvalidArgument`.
    pub fn parse(value: &str) -> Result<Self, DruidError> {
        let (parsed, profile, subprotocol_style) = if let Some(rest) = value.strip_prefix("rdbc:") {
            if rest.starts_with("//") {
                let parsed = url::Url::parse(value).map_err(|error| {
                    DruidError::InvalidArgument(format!("invalid RDBC URL: {error}"))
                })?;
                let profile = parsed
                    .host_str()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        DruidError::InvalidArgument(
                            "RDBC URL must contain a database profile".to_owned(),
                        )
                    })?
                    .to_owned();
                (parsed, profile, false)
            } else {
                let (profile, target) = rest.split_once("://").ok_or_else(|| {
                    DruidError::InvalidArgument(
                        "RDBC URL must use 'rdbc:<profile>://<endpoint>'".to_owned(),
                    )
                })?;
                if profile.is_empty() {
                    return Err(DruidError::InvalidArgument(
                        "RDBC URL must contain a database profile".to_owned(),
                    ));
                }
                let parsed = url::Url::parse(&format!("rdbc://{target}")).map_err(|error| {
                    DruidError::InvalidArgument(format!("invalid RDBC URL: {error}"))
                })?;
                (parsed, profile.to_owned(), true)
            }
        } else {
            let parsed = url::Url::parse(value).map_err(|error| {
                DruidError::InvalidArgument(format!("invalid RDBC URL: {error}"))
            })?;
            return if parsed.scheme() == "rdbc" {
                Err(DruidError::InvalidArgument(
                    "RDBC URL must use 'rdbc:<profile>://<endpoint>'".to_owned(),
                ))
            } else {
                Err(DruidError::InvalidArgument(
                    "RDBC URL scheme must be 'rdbc'".to_owned(),
                ))
            };
        };
        if parsed.username() != "" || parsed.password().is_some() || parsed.fragment().is_some() {
            return Err(DruidError::InvalidArgument(
                "RDBC URL forbids user-info and fragments; use connection properties".to_owned(),
            ));
        }
        let (endpoint, database) = if subprotocol_style {
            let host = parsed
                .host_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    DruidError::InvalidArgument("RDBC URL must contain an endpoint".to_owned())
                })?;
            let host = if host.contains(':') {
                format!("[{host}]")
            } else {
                host.to_owned()
            };
            let endpoint = parsed
                .port()
                .map_or(host.clone(), |port| format!("{host}:{port}"));
            let database = parsed
                .path_segments()
                .ok_or_else(|| DruidError::InvalidArgument("RDBC URL path is invalid".to_owned()))?
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("/");
            (endpoint, database)
        } else {
            let mut segments = parsed
                .path_segments()
                .ok_or_else(|| DruidError::InvalidArgument("RDBC URL path is invalid".to_owned()))?
                .filter(|value| !value.is_empty());
            let endpoint = segments.next().unwrap_or_default().to_owned();
            let database = segments.collect::<Vec<_>>().join("/");
            (endpoint, database)
        };
        let properties = parsed
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        Ok(Self {
            raw: value.to_owned(),
            profile,
            endpoint,
            database,
            properties,
            subprotocol_style,
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
        if self.subprotocol_style {
            format!("rdbc:{}://{}{suffix}", self.profile, self.endpoint)
        } else {
            format!("rdbc://{}/{}{suffix}", self.profile, self.endpoint)
        }
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
