use super::{
    database_profile_record::DatabaseProfileRecord, DatabaseProfileId, DriverCapabilities,
    DriverRegistryError, DriverRuntimeMode, DriverSupportStatus, ProtocolFamily, WallMode,
};
use druid::sql::DbType;

/// 经过清单校验的数据库产品档案。
#[derive(Debug, Clone)]
pub struct DatabaseProfile {
    id: DatabaseProfileId,
    display_name: String,
    db_type: DbType,
    protocol_family: ProtocolFamily,
    runtime_mode: DriverRuntimeMode,
    provider_id: String,
    default_port: Option<u16>,
    support_status: DriverSupportStatus,
    wall_mode: WallMode,
    delivery_phase: u8,
    validation_query: Option<String>,
    capabilities: DriverCapabilities,
}

impl DatabaseProfile {
    pub(crate) fn from_record(record: DatabaseProfileRecord) -> Result<Self, DriverRegistryError> {
        let id = DatabaseProfileId::new(record.id)?;
        let db_type = DbType::of(&record.db_type).ok_or_else(|| {
            DriverRegistryError::InvalidManifest(format!(
                "profile '{id}' references unknown Druid DbType '{}'",
                record.db_type
            ))
        })?;
        if record.display_name.trim().is_empty() || record.provider_id.trim().is_empty() {
            return Err(DriverRegistryError::InvalidManifest(format!(
                "profile '{id}' must define displayName and providerId"
            )));
        }
        if !(1..=3).contains(&record.delivery_phase) {
            return Err(DriverRegistryError::InvalidManifest(format!(
                "profile '{id}' has invalid deliveryPhase {}",
                record.delivery_phase
            )));
        }
        Ok(Self {
            id,
            display_name: record.display_name,
            db_type,
            protocol_family: record.protocol_family,
            runtime_mode: record.runtime_mode,
            provider_id: record.provider_id,
            default_port: record.default_port,
            support_status: record.support_status,
            wall_mode: record.wall_mode,
            delivery_phase: record.delivery_phase,
            validation_query: record.validation_query,
            capabilities: record.capabilities,
        })
    }

    #[must_use]
    pub fn id(&self) -> &DatabaseProfileId {
        &self.id
    }
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    #[must_use]
    pub const fn db_type(&self) -> DbType {
        self.db_type
    }
    #[must_use]
    pub const fn protocol_family(&self) -> ProtocolFamily {
        self.protocol_family
    }
    #[must_use]
    pub const fn runtime_mode(&self) -> DriverRuntimeMode {
        self.runtime_mode
    }
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }
    #[must_use]
    pub const fn default_port(&self) -> Option<u16> {
        self.default_port
    }
    #[must_use]
    pub const fn support_status(&self) -> DriverSupportStatus {
        self.support_status
    }
    #[must_use]
    pub const fn wall_mode(&self) -> WallMode {
        self.wall_mode
    }
    #[must_use]
    pub const fn delivery_phase(&self) -> u8 {
        self.delivery_phase
    }
    #[must_use]
    pub fn validation_query(&self) -> Option<&str> {
        self.validation_query.as_deref()
    }
    #[must_use]
    pub const fn capabilities(&self) -> DriverCapabilities {
        self.capabilities
    }
}
