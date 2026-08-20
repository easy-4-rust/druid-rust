use super::{
    DatabaseConnectionConfig, DatabaseProfile, DatabaseProfileId, DriverManifest,
    DriverRegistryError, DriverRuntimeMode, ProtocolFamily, ResolvedDatabaseDriver,
};
#[cfg(feature = "duckdb-native")]
use crate::duckdb::DuckDbConnectionFactory;
#[cfg(feature = "http-sql")]
use crate::http_sql::{HttpSqlConnectionFactory, HttpSqlProvider};
#[cfg(feature = "jdbc-agent")]
use crate::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
#[cfg(feature = "libsql-native")]
use crate::libsql::LibSqlConnectionFactory;
use crate::sqlx::SqlxConnectionFactory;
use druid_core::core::PhysicalConnectionFactory;
use druid_core::sql::RdbcUrl;
use std::collections::HashMap;
use std::sync::Arc;

/// 将产品档案解析为 Druid 物理连接工厂的只读注册中心。
#[derive(Debug, Clone)]
pub struct DruidDriverRegistry {
    manifest: DriverManifest,
    profile_indexes: HashMap<DatabaseProfileId, usize>,
    #[cfg(feature = "jdbc-agent")]
    jdbc_agent_options: Option<JdbcAgentOptions>,
}

impl DruidDriverRegistry {
    /// 加载内置 80 数据库产品目录。
    pub fn builtin() -> Result<Self, DriverRegistryError> {
        Self::from_manifest(DriverManifest::builtin()?)
    }

    /// 从已校验清单创建注册中心。
    pub fn from_manifest(manifest: DriverManifest) -> Result<Self, DriverRegistryError> {
        let profile_indexes = manifest
            .profiles()
            .iter()
            .enumerate()
            .map(|(index, profile)| (profile.id().clone(), index))
            .collect();
        Ok(Self {
            manifest,
            profile_indexes,
            #[cfg(feature = "jdbc-agent")]
            jdbc_agent_options: None,
        })
    }

    /// 显式安装 JDBC Agent 运行时；未调用时 JDBC 档案保持不可解析。
    #[cfg(feature = "jdbc-agent")]
    #[must_use]
    pub fn with_jdbc_agent(mut self, options: JdbcAgentOptions) -> Self {
        self.jdbc_agent_options = Some(options);
        self
    }

    /// 查询指定产品档案。
    pub fn profile(&self, id: &DatabaseProfileId) -> Result<&DatabaseProfile, DriverRegistryError> {
        self.profile_indexes
            .get(id)
            .and_then(|index| self.manifest.profiles().get(*index))
            .ok_or_else(|| DriverRegistryError::UnknownProfile(id.to_string()))
    }

    /// 返回清单顺序中的所有产品档案。
    pub fn profiles(&self) -> impl ExactSizeIterator<Item = &DatabaseProfile> {
        self.manifest.profiles().iter()
    }

    /// 返回目录版本。
    #[must_use]
    pub fn catalog_version(&self) -> &str {
        self.manifest.catalog_version()
    }

    /// 返回目录产品数量；该值不是公开支持数量。
    #[must_use]
    pub fn catalog_size(&self) -> usize {
        self.manifest.profiles().len()
    }

    /// 只统计具备 verified/certified 证据的产品数量。
    #[must_use]
    pub fn supported_count(&self) -> usize {
        self.profiles()
            .filter(|profile| profile.support_status().counts_as_supported())
            .count()
    }

    /// 将当前可用的原生 `SQLx` 档案解析为未池化连接工厂。
    pub fn resolve(
        &self,
        config: &DatabaseConnectionConfig,
    ) -> Result<ResolvedDatabaseDriver, DriverRegistryError> {
        let profile = self.profile(config.profile_id())?.clone();
        let (driver_url, display_url, properties) = if config.url().starts_with("rdbc:") {
            let rdbc_url =
                RdbcUrl::parse(config.url()).map_err(|_| DriverRegistryError::InvalidUrl {
                    profile: profile.id().to_string(),
                    url: "rdbc:<redacted>".to_owned(),
                })?;
            if rdbc_url.profile() != profile.id().as_str() {
                return Err(DriverRegistryError::InvalidUrl {
                    profile: profile.id().to_string(),
                    url: rdbc_url.redacted(),
                });
            }
            let mut properties = rdbc_url.properties().clone();
            properties.extend(config.properties().clone());
            (
                Self::rdbc_driver_url(&profile, &rdbc_url, &properties)?,
                rdbc_url.redacted(),
                properties,
            )
        } else {
            (
                config.url().to_owned(),
                config.url().to_owned(),
                config.properties().clone(),
            )
        };
        // Properties are consumed only by feature-gated native/JDBC factories.
        let _ = &properties;
        match profile.runtime_mode() {
            DriverRuntimeMode::Sqlx => {
                let driver_url = Self::normalize_sqlx_url(&profile, &driver_url)?;
                let factory: Arc<dyn PhysicalConnectionFactory> =
                    Arc::new(SqlxConnectionFactory::new(driver_url));
                Ok(ResolvedDatabaseDriver::new(
                    profile,
                    display_url.clone(),
                    factory,
                ))
            }
            DriverRuntimeMode::JdbcAgent => {
                #[cfg(not(feature = "jdbc-agent"))]
                {
                    return Err(DriverRegistryError::UnsupportedRuntime {
                        profile: profile.id().to_string(),
                        runtime: "JdbcAgent feature disabled".to_owned(),
                    });
                }
                #[cfg(feature = "jdbc-agent")]
                {
                    if !driver_url.starts_with("jdbc:") {
                        return Err(DriverRegistryError::InvalidUrl {
                            profile: profile.id().to_string(),
                            url: display_url.clone(),
                        });
                    }
                    let options = self.jdbc_agent_options.clone().ok_or_else(|| {
                        DriverRegistryError::UnsupportedRuntime {
                            profile: profile.id().to_string(),
                            runtime: "JdbcAgent".to_owned(),
                        }
                    })?;
                    let mut factory = JdbcAgentConnectionFactory::new(
                        &driver_url,
                        profile.validation_query().map(str::to_owned),
                        options,
                    );
                    for (name, value) in &properties {
                        factory = factory.property(name, value);
                    }
                    let factory: Arc<dyn PhysicalConnectionFactory> = Arc::new(factory);
                    Ok(ResolvedDatabaseDriver::new(
                        profile,
                        display_url.clone(),
                        factory,
                    ))
                }
            }
            DriverRuntimeMode::Native => {
                if profile.id().as_str() == "duckdb" {
                    #[cfg(feature = "duckdb-native")]
                    {
                        if !driver_url.starts_with("duckdb:") {
                            return Err(DriverRegistryError::InvalidUrl {
                                profile: profile.id().to_string(),
                                url: display_url.clone(),
                            });
                        }
                        let factory: Arc<dyn PhysicalConnectionFactory> =
                            Arc::new(DuckDbConnectionFactory::new(&driver_url));
                        return Ok(ResolvedDatabaseDriver::new(
                            profile,
                            display_url.clone(),
                            factory,
                        ));
                    }
                    #[cfg(not(feature = "duckdb-native"))]
                    return Err(DriverRegistryError::UnsupportedRuntime {
                        profile: profile.id().to_string(),
                        runtime: "DuckDB native feature disabled".to_owned(),
                    });
                }
                if profile.id().as_str() == "turso" {
                    #[cfg(feature = "libsql-native")]
                    {
                        if !driver_url.starts_with("libsql://")
                            && !driver_url.starts_with("https://")
                        {
                            return Err(DriverRegistryError::InvalidUrl {
                                profile: profile.id().to_string(),
                                url: display_url.clone(),
                            });
                        }
                        let mut factory = LibSqlConnectionFactory::new(&driver_url);
                        for (name, value) in &properties {
                            factory = factory.property(name, value);
                        }
                        let factory: Arc<dyn PhysicalConnectionFactory> = Arc::new(factory);
                        return Ok(ResolvedDatabaseDriver::new(
                            profile,
                            display_url.clone(),
                            factory,
                        ));
                    }
                    #[cfg(not(feature = "libsql-native"))]
                    return Err(DriverRegistryError::UnsupportedRuntime {
                        profile: profile.id().to_string(),
                        runtime: "Turso/libSQL native feature disabled".to_owned(),
                    });
                }
                Err(DriverRegistryError::UnsupportedRuntime {
                    profile: profile.id().to_string(),
                    runtime: format!("unknown native provider {}", profile.provider_id()),
                })
            }
            DriverRuntimeMode::HttpSql => {
                #[cfg(not(feature = "http-sql"))]
                {
                    return Err(DriverRegistryError::UnsupportedRuntime {
                        profile: profile.id().to_string(),
                        runtime: "HttpSql feature disabled".to_owned(),
                    });
                }
                #[cfg(feature = "http-sql")]
                {
                    if !driver_url.starts_with("http://") && !driver_url.starts_with("https://") {
                        return Err(DriverRegistryError::InvalidUrl {
                            profile: profile.id().to_string(),
                            url: display_url.clone(),
                        });
                    }
                    let provider = HttpSqlProvider::from_provider_id(profile.provider_id())
                        .ok_or_else(|| DriverRegistryError::UnsupportedRuntime {
                            profile: profile.id().to_string(),
                            runtime: format!("unknown HTTP SQL provider {}", profile.provider_id()),
                        })?;
                    let mut factory = HttpSqlConnectionFactory::new(provider, &driver_url);
                    for (name, value) in &properties {
                        factory = factory.property(name, value);
                    }
                    let factory: Arc<dyn PhysicalConnectionFactory> = Arc::new(factory);
                    Ok(ResolvedDatabaseDriver::new(profile, display_url, factory))
                }
            }
        }
    }

    fn rdbc_driver_url(
        profile: &DatabaseProfile,
        rdbc_url: &RdbcUrl,
        properties: &HashMap<String, String>,
    ) -> Result<String, DriverRegistryError> {
        let invalid = || DriverRegistryError::InvalidUrl {
            profile: profile.id().to_string(),
            url: rdbc_url.redacted(),
        };
        let network = |scheme: &str| rdbc_url.network_url(scheme).map_err(|_| invalid());
        match profile.runtime_mode() {
            DriverRuntimeMode::Sqlx => match profile.protocol_family() {
                ProtocolFamily::MySql => Self::sqlx_network_url(
                    profile,
                    &network("mysql")?,
                    properties,
                    &rdbc_url.redacted(),
                ),
                ProtocolFamily::PostgreSql => Self::sqlx_network_url(
                    profile,
                    &network("postgresql")?,
                    properties,
                    &rdbc_url.redacted(),
                ),
                ProtocolFamily::SQLite => {
                    if rdbc_url.endpoint() == ":memory:" {
                        Ok("sqlite::memory:".to_owned())
                    } else {
                        let path = [rdbc_url.endpoint(), rdbc_url.database()]
                            .into_iter()
                            .filter(|value| !value.is_empty())
                            .collect::<Vec<_>>()
                            .join("/");
                        Ok(format!("sqlite://{path}"))
                    }
                }
                _ => Err(invalid()),
            },
            DriverRuntimeMode::Native if profile.id().as_str() == "duckdb" => {
                let path = [rdbc_url.endpoint(), rdbc_url.database()]
                    .into_iter()
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("/");
                (!path.is_empty())
                    .then(|| format!("duckdb:{path}"))
                    .ok_or_else(invalid)
            }
            DriverRuntimeMode::Native if profile.id().as_str() == "turso" => network("libsql"),
            DriverRuntimeMode::HttpSql => {
                let scheme = if rdbc_url.property("tls") == Some("false") {
                    "http"
                } else {
                    "https"
                };
                network(scheme)
            }
            DriverRuntimeMode::JdbcAgent => {
                let endpoint = rdbc_url.endpoint();
                if endpoint.is_empty() {
                    return Err(invalid());
                }
                let database = rdbc_url.database();
                let suffix = if database.is_empty() {
                    String::new()
                } else {
                    format!("/{database}")
                };
                let url = match profile.protocol_family() {
                    ProtocolFamily::Oracle => format!("jdbc:oracle:thin:@//{endpoint}{suffix}"),
                    ProtocolFamily::SqlServer => {
                        if database.is_empty() {
                            format!("jdbc:sqlserver://{endpoint}")
                        } else {
                            format!("jdbc:sqlserver://{endpoint};databaseName={database}")
                        }
                    }
                    ProtocolFamily::SQLite => format!("jdbc:sqlite:{endpoint}{suffix}"),
                    _ => match profile.id().as_str() {
                        "h2" if rdbc_url.property("mode") == Some("memory") => {
                            format!("jdbc:h2:mem:{endpoint}")
                        }
                        "h2" => format!("jdbc:h2:tcp://{endpoint}{suffix}"),
                        "hsqldb" if rdbc_url.property("mode") == Some("memory") => {
                            format!("jdbc:hsqldb:mem:{endpoint}")
                        }
                        "hsqldb" => format!("jdbc:hsqldb:hsql://{endpoint}{suffix}"),
                        "derby" => format!("jdbc:derby://{endpoint}{suffix}"),
                        id => format!("jdbc:{id}://{endpoint}{suffix}"),
                    },
                };
                Ok(url)
            }
            DriverRuntimeMode::Native => Err(invalid()),
        }
    }

    fn sqlx_network_url(
        profile: &DatabaseProfile,
        network_url: &str,
        properties: &HashMap<String, String>,
        display_url: &str,
    ) -> Result<String, DriverRegistryError> {
        let invalid = || DriverRegistryError::InvalidUrl {
            profile: profile.id().to_string(),
            url: display_url.to_owned(),
        };
        let mut parsed = url::Url::parse(network_url).map_err(|_| invalid())?;
        if let Some(user_name) = properties.get("user") {
            parsed.set_username(user_name).map_err(|()| invalid())?;
        }
        if let Some(password) = properties.get("password") {
            parsed
                .set_password(Some(password))
                .map_err(|()| invalid())?;
        }

        let mut effective_properties = properties.clone();
        if profile.protocol_family() == ProtocolFamily::MySql {
            if !effective_properties.contains_key("charset") {
                if let Some(character_encoding) = properties.get("characterEncoding") {
                    effective_properties.insert("charset".to_owned(), character_encoding.clone());
                }
            }
            if !effective_properties.contains_key("sslmode")
                && !effective_properties.contains_key("ssl-mode")
            {
                if let Some(use_ssl) = properties.get("useSSL") {
                    let ssl_mode = if use_ssl.eq_ignore_ascii_case("false") {
                        "DISABLED"
                    } else if use_ssl.eq_ignore_ascii_case("true") {
                        "PREFERRED"
                    } else {
                        return Err(invalid());
                    };
                    effective_properties.insert("ssl-mode".to_owned(), ssl_mode.to_owned());
                }
            }
            if !effective_properties.contains_key("timezone")
                && !effective_properties.contains_key("time-zone")
            {
                if let Some(server_timezone) = properties.get("serverTimezone") {
                    effective_properties.insert(
                        "timezone".to_owned(),
                        Self::sqlx_mysql_timezone(server_timezone),
                    );
                }
            }
        }

        let mut driver_properties = effective_properties
            .iter()
            .filter(|(name, _)| name.as_str() != "user" && name.as_str() != "password")
            .collect::<Vec<_>>();
        driver_properties.sort_by_key(|(name, _)| *name);
        if !driver_properties.is_empty() {
            let mut query = parsed.query_pairs_mut();
            for (name, value) in driver_properties {
                query.append_pair(name, value);
            }
        }
        Ok(parsed.into())
    }

    fn sqlx_mysql_timezone(server_timezone: &str) -> String {
        let Some(offset) = server_timezone
            .strip_prefix("GMT")
            .or_else(|| server_timezone.strip_prefix("UTC"))
        else {
            return server_timezone.to_owned();
        };
        if offset.is_empty() {
            return "+00:00".to_owned();
        }
        let (sign, digits) = match offset.as_bytes().first() {
            Some(b'+') => ('+', &offset[1..]),
            Some(b'-') => ('-', &offset[1..]),
            _ => return server_timezone.to_owned(),
        };
        let (hours, minutes) = digits.split_once(':').map_or((digits, "0"), |parts| parts);
        let Ok(hours) = hours.parse::<u8>() else {
            return server_timezone.to_owned();
        };
        let Ok(minutes) = minutes.parse::<u8>() else {
            return server_timezone.to_owned();
        };
        if hours > 14 || minutes > 59 || (hours == 14 && minutes != 0) {
            return server_timezone.to_owned();
        }
        format!("{sign}{hours:02}:{minutes:02}")
    }

    fn normalize_sqlx_url(
        profile: &DatabaseProfile,
        url: &str,
    ) -> Result<String, DriverRegistryError> {
        let normalized = match profile.protocol_family() {
            ProtocolFamily::MySql if url.starts_with("mysql://") => Some(url.to_owned()),
            ProtocolFamily::MySql if url.starts_with("jdbc:mysql://") => {
                url.strip_prefix("jdbc:").map(str::to_owned)
            }
            ProtocolFamily::MySql if url.starts_with("jdbc:mariadb://") => url
                .strip_prefix("jdbc:mariadb://")
                .map(|suffix| format!("mysql://{suffix}")),
            ProtocolFamily::PostgreSql
                if url.starts_with("postgres://") || url.starts_with("postgresql://") =>
            {
                Some(url.to_owned())
            }
            ProtocolFamily::PostgreSql if url.starts_with("jdbc:postgresql://") => {
                url.strip_prefix("jdbc:").map(str::to_owned)
            }
            ProtocolFamily::SQLite if url.starts_with("sqlite:") => Some(url.to_owned()),
            ProtocolFamily::SQLite if url.starts_with("jdbc:sqlite:") => {
                url.strip_prefix("jdbc:").map(str::to_owned)
            }
            ProtocolFamily::Oracle
            | ProtocolFamily::SqlServer
            | ProtocolFamily::Embedded
            | ProtocolFamily::Jdbc
            | ProtocolFamily::HttpSql
            | ProtocolFamily::MySql
            | ProtocolFamily::PostgreSql
            | ProtocolFamily::SQLite => None,
        };
        normalized.ok_or_else(|| DriverRegistryError::InvalidUrl {
            profile: profile.id().to_string(),
            url: url.to_owned(),
        })
    }
}
