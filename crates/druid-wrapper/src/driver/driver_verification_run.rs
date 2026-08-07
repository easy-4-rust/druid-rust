use super::DriverRuntimeMode;
use serde::Deserialize;

/// 一次数据库产品契约验证的可审计运行记录。
///
/// 记录必须绑定真实数据库版本、目标平台、源代码修订和外部证据引用；仅有平台名称
/// 或本地静态测试不能构成 `Verified` 证据。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DriverVerificationRun {
    target: String,
    database_version: String,
    rust_version: String,
    #[serde(default)]
    java_versions: Vec<u16>,
    runtime_mode: DriverRuntimeMode,
    installation_paths: Vec<String>,
    contract_checks: Vec<String>,
    source_revision: String,
    evidence_ref: String,
    passed_at: String,
    artifact_sha256: Option<String>,
}

impl DriverVerificationRun {
    pub(crate) fn validates(
        &self,
        expected_runtime_mode: DriverRuntimeMode,
        expected_contract_artifact: Option<&str>,
    ) -> bool {
        self.runtime_mode == expected_runtime_mode
            && !self.target.trim().is_empty()
            && !self.database_version.trim().is_empty()
            && !self.rust_version.trim().is_empty()
            && self.validates_runtime_paths()
            && self.validates_contract_checks()
            && is_source_revision(&self.source_revision)
            && is_evidence_reference(&self.evidence_ref)
            && is_rfc3339_like(&self.passed_at)
            && self.artifact_sha256.as_deref().is_none_or(is_sha256)
            && expected_contract_artifact.is_none_or(|expected| {
                self.artifact_sha256
                    .as_deref()
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
            })
    }

    fn validates_runtime_paths(&self) -> bool {
        let contains = |required: &str| {
            self.installation_paths
                .iter()
                .any(|path| path.eq_ignore_ascii_case(required))
        };
        match self.runtime_mode {
            DriverRuntimeMode::Sqlx | DriverRuntimeMode::Native => contains("native"),
            DriverRuntimeMode::JdbcAgent => {
                self.java_versions.contains(&17)
                    && contains("jdbc-agent")
                    && contains("offline-preinstalled")
                    && contains("explicit-install")
            }
            DriverRuntimeMode::HttpSql => contains("http-sql"),
        }
    }

    fn validates_contract_checks(&self) -> bool {
        const REQUIRED: [&str; 13] = [
            "connection-lifecycle",
            "validation",
            "crud-ddl",
            "scalar-types",
            "prepared-and-batch",
            "transactions",
            "state-reset",
            "capabilities",
            "error-classification",
            "timeout-cancel",
            "database-restart",
            "concurrency-leak-shutdown",
            "no-pool-in-pool",
        ];
        let has = |required: &str| {
            self.contract_checks
                .iter()
                .any(|check| check.eq_ignore_ascii_case(required))
        };
        REQUIRED.iter().all(|required| has(required))
            && (self.runtime_mode != DriverRuntimeMode::JdbcAgent
                || ["agent-crash", "protocol-failure"]
                    .iter()
                    .all(|required| has(required)))
    }

    /// 返回 Rust 编译目标三元组。
    #[must_use]
    pub fn target(&self) -> &str {
        &self.target
    }

    /// 返回真实运行的数据库版本。
    #[must_use]
    pub fn database_version(&self) -> &str {
        &self.database_version
    }

    /// 返回该运行实际采用的 Rust 工具链标签。
    #[must_use]
    pub fn rust_version(&self) -> &str {
        &self.rust_version
    }

    /// 返回该运行覆盖的 Java 主版本。
    #[must_use]
    pub fn java_versions(&self) -> &[u16] {
        &self.java_versions
    }

    /// 返回本次验证采用的运行模式。
    #[must_use]
    pub const fn runtime_mode(&self) -> DriverRuntimeMode {
        self.runtime_mode
    }

    /// 返回本次验证覆盖的安装路径。
    #[must_use]
    pub fn installation_paths(&self) -> &[String] {
        &self.installation_paths
    }

    /// 返回本次运行实际覆盖的统一契约检查项。
    #[must_use]
    pub fn contract_checks(&self) -> &[String] {
        &self.contract_checks
    }

    /// 返回被验证的 Git 源代码修订。
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    /// 返回不可变 CI 运行或 vendor lab 报告引用。
    #[must_use]
    pub fn evidence_ref(&self) -> &str {
        &self.evidence_ref
    }

    /// 返回本次运行通过的 RFC 3339 时间。
    #[must_use]
    pub fn passed_at(&self) -> &str {
        &self.passed_at
    }

    /// 返回本次运行绑定的驱动工件 SHA-256。
    #[must_use]
    pub fn artifact_sha256(&self) -> Option<&str> {
        self.artifact_sha256.as_deref()
    }
}

pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_source_revision(value: &str) -> bool {
    (40..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_evidence_reference(value: &str) -> bool {
    ["https://", "file:", "urn:"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

fn is_rfc3339_like(value: &str) -> bool {
    value.len() >= 20 && value.contains('T') && (value.ends_with('Z') || value.contains('+'))
}
