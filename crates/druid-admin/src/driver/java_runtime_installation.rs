use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 已校验且可由 JDBC Agent 使用的 Java 17+ 运行时引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JavaRuntimeInstallation {
    java_home: PathBuf,
    java_program: PathBuf,
    sha256: String,
    major_version: u16,
    source: String,
    installed_at_epoch_millis: i64,
}

impl JavaRuntimeInstallation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        java_home: PathBuf,
        java_program: PathBuf,
        sha256: String,
        major_version: u16,
        source: String,
        installed_at_epoch_millis: i64,
    ) -> Self {
        Self {
            java_home,
            java_program,
            sha256,
            major_version,
            source,
            installed_at_epoch_millis,
        }
    }

    /// 返回 Java home。
    #[must_use]
    pub fn java_home(&self) -> &Path {
        &self.java_home
    }

    /// 返回不经 shell 执行的 Java 程序路径。
    #[must_use]
    pub fn java_program(&self) -> &Path {
        &self.java_program
    }

    /// 返回 Java 可执行文件 SHA-256。
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// 返回 Java 主版本。
    #[must_use]
    pub const fn major_version(&self) -> u16 {
        self.major_version
    }

    pub(crate) const fn installed_at_epoch_millis(&self) -> i64 {
        self.installed_at_epoch_millis
    }
}
