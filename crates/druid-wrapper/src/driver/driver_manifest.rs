use super::{
    driver_manifest_record::DriverManifestRecord, DatabaseProfile, DriverRegistryError,
    DriverRuntimeMode, ProtocolFamily,
};
use std::collections::HashSet;

/// 经过 schema、类型和唯一性校验的数据库驱动清单。
#[derive(Debug, Clone)]
pub struct DriverManifest {
    schema_version: u32,
    catalog_version: String,
    profiles: Vec<DatabaseProfile>,
}

impl DriverManifest {
    pub(crate) const CURRENT_SCHEMA_VERSION: u32 = 3;

    /// 加载内置的数据库产品目录。
    pub fn builtin() -> Result<Self, DriverRegistryError> {
        Self::from_json(include_str!("../../assets/database-drivers.manifest.json"))
    }

    /// 从 JSON 加载清单，并拒绝未知字段、重复标识和未知 Druid 方言。
    pub fn from_json(json: &str) -> Result<Self, DriverRegistryError> {
        let record: DriverManifestRecord = serde_json::from_str(json)
            .map_err(|error| DriverRegistryError::InvalidManifest(error.to_string()))?;
        if record.schema_version != Self::CURRENT_SCHEMA_VERSION {
            return Err(DriverRegistryError::InvalidManifest(format!(
                "unsupported schemaVersion {}, expected {}",
                record.schema_version,
                Self::CURRENT_SCHEMA_VERSION
            )));
        }
        if record.catalog_version.trim().is_empty() {
            return Err(DriverRegistryError::InvalidManifest(
                "catalogVersion must not be empty".to_owned(),
            ));
        }
        let profiles = record
            .profiles
            .into_iter()
            .map(DatabaseProfile::from_record)
            .collect::<Result<Vec<_>, _>>()?;
        let mut ids = HashSet::with_capacity(profiles.len());
        for profile in &profiles {
            if !ids.insert(profile.id().clone()) {
                return Err(DriverRegistryError::InvalidManifest(format!(
                    "duplicate profile id '{}'",
                    profile.id()
                )));
            }
            Self::validate_profile_contract(profile)?;
        }
        Ok(Self {
            schema_version: record.schema_version,
            catalog_version: record.catalog_version,
            profiles,
        })
    }

    fn validate_profile_contract(profile: &DatabaseProfile) -> Result<(), DriverRegistryError> {
        let compatible = match profile.runtime_mode() {
            DriverRuntimeMode::Sqlx => {
                profile.provider_id() == "sqlx"
                    && matches!(
                        profile.protocol_family(),
                        ProtocolFamily::MySql | ProtocolFamily::PostgreSql | ProtocolFamily::SQLite
                    )
            }
            DriverRuntimeMode::JdbcAgent => {
                profile.provider_id() == "jdbc-agent"
                    && profile.protocol_family() != ProtocolFamily::HttpSql
            }
            DriverRuntimeMode::HttpSql => profile.protocol_family() == ProtocolFamily::HttpSql,
            DriverRuntimeMode::Native => profile.provider_id() != "jdbc-agent",
        };
        if !compatible {
            return Err(DriverRegistryError::InvalidManifest(format!(
                "profile '{}' has incompatible runtimeMode, providerId and protocolFamily",
                profile.id()
            )));
        }
        if profile.support_status().counts_as_supported() {
            let capabilities = profile.capabilities();
            if !capabilities.query
                || !capabilities.update
                || !capabilities.prepared_statements
                || !capabilities.transactions
            {
                return Err(DriverRegistryError::InvalidManifest(format!(
                    "verified profile '{}' does not satisfy the minimum capability contract",
                    profile.id()
                )));
            }
            if profile
                .evidence()
                .is_none_or(|evidence| !evidence.validates_support_contract(profile.runtime_mode()))
            {
                return Err(DriverRegistryError::InvalidManifest(format!(
                    "verified profile '{}' lacks Linux/macOS/Windows evidence",
                    profile.id()
                )));
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
    #[must_use]
    pub fn catalog_version(&self) -> &str {
        &self.catalog_version
    }
    #[must_use]
    pub fn profiles(&self) -> &[DatabaseProfile] {
        &self.profiles
    }
}
