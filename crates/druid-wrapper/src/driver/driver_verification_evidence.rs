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
}

impl DriverVerificationEvidence {
    pub(crate) fn validates_support_contract(&self) -> bool {
        !self.contract_version.trim().is_empty()
            && !self.tested_at.trim().is_empty()
            && ["linux", "macos", "windows"].iter().all(|required| {
                self.platforms
                    .iter()
                    .any(|platform| platform.eq_ignore_ascii_case(required))
            })
    }

    /// 返回验证合同版本。
    #[must_use]
    pub fn contract_version(&self) -> &str {
        &self.contract_version
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
}
