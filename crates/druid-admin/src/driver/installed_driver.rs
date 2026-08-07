use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 内容寻址保存的已安装 JDBC 驱动或 Agent 工件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledDriver {
    profile_id: String,
    file_name: String,
    #[serde(default)]
    artifact_version: String,
    sha256: String,
    path: PathBuf,
    source: String,
    #[serde(default = "InstalledDriver::default_license")]
    license: String,
    #[serde(default)]
    driver_class: Option<String>,
    #[serde(default)]
    jar_files: Vec<String>,
    #[serde(default = "InstalledDriver::default_java_version")]
    minimum_java_version: u16,
    installed_at_epoch_millis: i64,
}

impl InstalledDriver {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        profile_id: String,
        file_name: String,
        sha256: String,
        path: PathBuf,
        source: String,
        license: String,
        driver_class: Option<String>,
        installed_at_epoch_millis: i64,
    ) -> Self {
        Self {
            profile_id,
            artifact_version: sha256.clone(),
            jar_files: vec![file_name.clone()],
            file_name,
            sha256,
            path,
            source,
            license,
            driver_class,
            minimum_java_version: 17,
            installed_at_epoch_millis,
        }
    }

    /// 返回数据库产品 ID；Agent 本体使用 `jdbc-agent`。
    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// 返回安装后的绝对或根目录相对路径。
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回内容 SHA-256。
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// 返回不可变的制品版本；默认使用内容 SHA-256。
    #[must_use]
    pub fn artifact_version(&self) -> &str {
        &self.artifact_version
    }

    /// 返回 SPDX 许可证或 `NOASSERTION`。
    #[must_use]
    pub fn license(&self) -> &str {
        &self.license
    }

    /// 返回可选 JDBC driver class。
    #[must_use]
    pub fn driver_class(&self) -> Option<&str> {
        self.driver_class.as_deref()
    }

    /// 返回该版本固定的 Jar 文件清单。
    #[must_use]
    pub fn jar_files(&self) -> &[String] {
        &self.jar_files
    }

    /// 返回运行该制品要求的最低 Java 主版本。
    #[must_use]
    pub const fn minimum_java_version(&self) -> u16 {
        self.minimum_java_version
    }

    /// 返回安装时间戳。
    #[must_use]
    pub const fn installed_at_epoch_millis(&self) -> i64 {
        self.installed_at_epoch_millis
    }

    pub(crate) fn reactivated(&self, installed_at_epoch_millis: i64) -> Self {
        let mut installation = self.clone();
        installation.installed_at_epoch_millis = installed_at_epoch_millis;
        installation
    }

    fn default_license() -> String {
        "NOASSERTION".to_owned()
    }

    const fn default_java_version() -> u16 {
        17
    }
}
