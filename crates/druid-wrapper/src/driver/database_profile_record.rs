use super::{DriverCapabilities, DriverRuntimeMode, DriverSupportStatus, ProtocolFamily, WallMode};
use serde::Deserialize;

/// 驱动清单中的单个数据库产品档案记录。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DatabaseProfileRecord {
    pub id: String,
    pub display_name: String,
    pub db_type: String,
    pub protocol_family: ProtocolFamily,
    pub runtime_mode: DriverRuntimeMode,
    pub provider_id: String,
    pub default_port: Option<u16>,
    pub support_status: DriverSupportStatus,
    pub wall_mode: WallMode,
    pub delivery_phase: u8,
    pub validation_query: Option<String>,
    #[serde(default)]
    pub capabilities: DriverCapabilities,
}
