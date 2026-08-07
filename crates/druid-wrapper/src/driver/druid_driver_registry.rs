use super::{
    DatabaseConnectionConfig, DatabaseProfile, DatabaseProfileId, DriverManifest,
    DriverRegistryError, DriverRuntimeMode, ProtocolFamily, ResolvedDatabaseDriver,
};
#[cfg(feature = "jdbc-agent")]
use crate::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use crate::sqlx::SqlxConnectionFactory;
use druid::core::PhysicalConnectionFactory;
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
        match profile.runtime_mode() {
            DriverRuntimeMode::Sqlx => {
                let driver_url = Self::normalize_sqlx_url(&profile, config.url())?;
                let factory: Arc<dyn PhysicalConnectionFactory> =
                    Arc::new(SqlxConnectionFactory::new(driver_url));
                Ok(ResolvedDatabaseDriver::new(
                    profile,
                    config.url().to_owned(),
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
                    if !config.url().starts_with("jdbc:") {
                        return Err(DriverRegistryError::InvalidUrl {
                            profile: profile.id().to_string(),
                            url: config.url().to_owned(),
                        });
                    }
                    let options = self.jdbc_agent_options.clone().ok_or_else(|| {
                        DriverRegistryError::UnsupportedRuntime {
                            profile: profile.id().to_string(),
                            runtime: "JdbcAgent".to_owned(),
                        }
                    })?;
                    let mut factory = JdbcAgentConnectionFactory::new(
                        config.url(),
                        profile.validation_query().map(str::to_owned),
                        options,
                    );
                    for (name, value) in config.properties() {
                        factory = factory.property(name, value);
                    }
                    let factory: Arc<dyn PhysicalConnectionFactory> = Arc::new(factory);
                    Ok(ResolvedDatabaseDriver::new(
                        profile,
                        config.url().to_owned(),
                        factory,
                    ))
                }
            }
            runtime => Err(DriverRegistryError::UnsupportedRuntime {
                profile: profile.id().to_string(),
                runtime: format!("{runtime:?}"),
            }),
        }
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
