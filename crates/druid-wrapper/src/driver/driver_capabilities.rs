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
    pub multiple_results: bool,
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
    pub blob: bool,
    pub clob: bool,
    pub n_clob: bool,
    pub array: bool,
    pub sql_xml: bool,
}

impl DriverCapabilities {
    /// 判断清单是否尚未声明任何产品能力。
    #[must_use]
    pub const fn is_empty(self) -> bool {
        !self.query
            && !self.update
            && !self.prepared_statements
            && !self.batch
            && !self.generated_keys
            && !self.multiple_results
            && !self.transactions
            && !self.savepoints
            && !self.auto_commit
            && !self.read_only
            && !self.transaction_isolation
            && !self.catalog
            && !self.schema
            && !self.metadata
            && !self.cancellation
            && !self.paged_results
            && !self.lob
            && !self.blob
            && !self.clob
            && !self.n_clob
            && !self.array
            && !self.sql_xml
    }
}
