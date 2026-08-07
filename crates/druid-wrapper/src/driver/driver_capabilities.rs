use serde::Deserialize;

/// 驱动产品能力矩阵；未声明的能力保持关闭。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools)]
pub struct DriverCapabilities {
    pub query: bool,
    pub update: bool,
    pub prepared_statements: bool,
    pub batch: bool,
    pub generated_keys: bool,
    pub transactions: bool,
    pub savepoints: bool,
    pub auto_commit: bool,
    pub read_only: bool,
    pub transaction_isolation: bool,
    pub catalog: bool,
    pub schema: bool,
    pub metadata: bool,
    pub cancellation: bool,
    pub paged_results: bool,
    pub lob: bool,
    pub sql_xml: bool,
}
