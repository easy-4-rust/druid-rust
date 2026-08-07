use serde::Deserialize;

use super::DriverRuntimeMode;

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

    /// 为未逐项覆盖的目录项提供“当前 Adapter 能证明”的运行时基线。
    ///
    /// 该基线只描述 Druid 适配层能力，不把产品标记为 Verified；产品仍必须通过
    /// 五平台真实合同才能计入公开支持数。
    #[must_use]
    pub fn runtime_baseline(runtime_mode: DriverRuntimeMode, profile_id: &str) -> Self {
        match runtime_mode {
            DriverRuntimeMode::Sqlx => Self {
                query: true,
                update: true,
                prepared_statements: true,
                batch: true,
                generated_keys: true,
                multiple_results: false,
                transactions: true,
                savepoints: true,
                auto_commit: true,
                read_only: false,
                transaction_isolation: false,
                catalog: false,
                schema: false,
                metadata: true,
                cancellation: true,
                paged_results: false,
                lob: false,
                blob: false,
                clob: false,
                n_clob: false,
                array: false,
                sql_xml: false,
            },
            DriverRuntimeMode::JdbcAgent => Self {
                query: true,
                update: true,
                prepared_statements: true,
                batch: true,
                generated_keys: profile_id == "turso",
                multiple_results: false,
                transactions: true,
                savepoints: false,
                auto_commit: true,
                read_only: true,
                transaction_isolation: true,
                catalog: true,
                schema: true,
                metadata: true,
                cancellation: true,
                paged_results: true,
                lob: false,
                blob: false,
                clob: false,
                n_clob: false,
                array: false,
                sql_xml: false,
            },
            DriverRuntimeMode::HttpSql => Self {
                query: true,
                update: true,
                prepared_statements: true,
                batch: true,
                generated_keys: true,
                multiple_results: false,
                transactions: false,
                savepoints: false,
                auto_commit: false,
                read_only: false,
                transaction_isolation: false,
                catalog: false,
                schema: false,
                metadata: true,
                cancellation: true,
                paged_results: false,
                lob: false,
                blob: false,
                clob: false,
                n_clob: false,
                array: false,
                sql_xml: false,
            },
            DriverRuntimeMode::Native => Self {
                query: true,
                update: true,
                prepared_statements: true,
                batch: true,
                generated_keys: true,
                multiple_results: false,
                transactions: true,
                savepoints: profile_id == "turso",
                auto_commit: true,
                read_only: false,
                transaction_isolation: false,
                catalog: false,
                schema: false,
                metadata: true,
                cancellation: true,
                paged_results: false,
                lob: false,
                blob: true,
                clob: false,
                n_clob: false,
                array: false,
                sql_xml: false,
            },
        }
    }

    /// 将产品清单显式开启项叠加到 Adapter 运行时基线。
    #[must_use]
    pub const fn merged_with(self, declared: Self) -> Self {
        Self {
            query: self.query || declared.query,
            update: self.update || declared.update,
            prepared_statements: self.prepared_statements || declared.prepared_statements,
            batch: self.batch || declared.batch,
            generated_keys: self.generated_keys || declared.generated_keys,
            multiple_results: self.multiple_results || declared.multiple_results,
            transactions: self.transactions || declared.transactions,
            savepoints: self.savepoints || declared.savepoints,
            auto_commit: self.auto_commit || declared.auto_commit,
            read_only: self.read_only || declared.read_only,
            transaction_isolation: self.transaction_isolation || declared.transaction_isolation,
            catalog: self.catalog || declared.catalog,
            schema: self.schema || declared.schema,
            metadata: self.metadata || declared.metadata,
            cancellation: self.cancellation || declared.cancellation,
            paged_results: self.paged_results || declared.paged_results,
            lob: self.lob || declared.lob,
            blob: self.blob || declared.blob,
            clob: self.clob || declared.clob,
            n_clob: self.n_clob || declared.n_clob,
            array: self.array || declared.array,
            sql_xml: self.sql_xml || declared.sql_xml,
        }
    }
}
