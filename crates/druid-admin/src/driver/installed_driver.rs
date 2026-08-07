use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 内容寻址保存的已安装 JDBC 驱动或 Agent 工件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstalledDriver {
    profile_id: String,
    file_name: String,
    sha256: String,
    path: PathBuf,
    source: String,
    installed_at_epoch_millis: i64,
}

impl InstalledDriver {
    pub(crate) fn new(
        profile_id: String,
        file_name: String,
        sha256: String,
        path: PathBuf,
        source: String,
        installed_at_epoch_millis: i64,
    ) -> Self {
        Self {
            profile_id,
            file_name,
            sha256,
            path,
            source,
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

    /// 返回安装时间戳。
    #[must_use]
    pub const fn installed_at_epoch_millis(&self) -> i64 {
        self.installed_at_epoch_millis
    }
}
