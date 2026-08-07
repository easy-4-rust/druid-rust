use super::{driver_verification_run::is_sha256, DriverRuntimeMode, DriverVerificationRun};
use serde::Deserialize;

/// 数据库产品通过支持门禁时保留的可审计证据摘要。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DriverVerificationEvidence {
    contract_version: String,
    tested_at: String,
    platforms: Vec<String>,
    #[serde(default)]
    java_versions: Vec<u16>,
    artifact_sha256: Option<String>,
    runs: Vec<DriverVerificationRun>,
}

impl DriverVerificationEvidence {
    pub(crate) fn validates_support_contract(&self, runtime_mode: DriverRuntimeMode) -> bool {
        const REQUIRED_TARGETS: [&str; 5] = [
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ];
        !self.contract_version.trim().is_empty()
            && !self.tested_at.trim().is_empty()
            && self.artifact_sha256.as_deref().is_none_or(is_sha256)
            && ["linux", "macos", "windows"].iter().all(|required| {
                self.platforms
                    .iter()
                    .any(|platform| platform.eq_ignore_ascii_case(required))
            })
            && !self.runs.is_empty()
            && self
                .runs
                .iter()
                .all(|run| run.validates(runtime_mode, self.artifact_sha256.as_deref()))
            && REQUIRED_TARGETS.iter().all(|required| {
                ["1.95", "default"].iter().all(|rust_version| {
                    self.runs
                        .iter()
                        .any(|run| run.target() == *required && run.rust_version() == *rust_version)
                })
            })
    }

    /// 返回验证合同版本。
    #[must_use]
    pub fn contract_version(&self) -> &str {
        &self.contract_version
    }

    /// 返回证据集合生成时的 RFC 3339 时间。
    #[must_use]
    pub fn tested_at(&self) -> &str {
        &self.tested_at
    }

    /// 返回证据覆盖的平台。
    #[must_use]
    pub fn platforms(&self) -> &[String] {
        &self.platforms
    }

    /// 返回证据覆盖的 Java 主版本。
    #[must_use]
    pub fn java_versions(&self) -> &[u16] {
        &self.java_versions
    }

    /// 返回被验证工件的 SHA-256。
    #[must_use]
    pub fn artifact_sha256(&self) -> Option<&str> {
        self.artifact_sha256.as_deref()
    }

    /// 返回真实数据库契约的逐目标运行记录。
    #[must_use]
    pub fn runs(&self) -> &[DriverVerificationRun] {
        &self.runs
    }
}
