use super::{
    DriverCapabilities, DriverRuntimeMode, DriverSupportStatus, DriverVerificationEvidence,
    ProtocolFamily, WallMode,
};
use serde::Deserialize;

/// 驱动清单中的单个数据库产品档案记录。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DatabaseProfileRecord {
    pub profile_id: String,
    pub display_name: String,
    pub db_type: String,
    pub protocol_family: ProtocolFamily,
    pub runtime_mode: DriverRuntimeMode,
    pub provider_id: String,
    pub artifact_id: String,
    pub artifact_version: Option<String>,
    pub driver_class: Option<String>,
    pub default_port: Option<u16>,
    pub support_status: DriverSupportStatus,
    pub wall_mode: WallMode,
    pub delivery_phase: u8,
    pub validation_query: Option<String>,
    pub reset_sql: Option<String>,
    pub exception_sorter: String,
    pub evidence: Option<DriverVerificationEvidence>,
    #[serde(default)]
    pub capabilities: DriverCapabilities,
}
