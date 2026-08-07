use super::database_profile_record::DatabaseProfileRecord;
use serde::Deserialize;

/// 驱动清单的反序列化边界。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct DriverManifestRecord {
    pub schema_version: u32,
    pub catalog_version: String,
    pub profiles: Vec<DatabaseProfileRecord>,
}
