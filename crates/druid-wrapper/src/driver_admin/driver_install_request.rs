use std::path::PathBuf;

/// 从本地文件显式安装单个 JDBC 驱动的请求。
#[derive(Debug, Clone)]
pub struct DriverInstallRequest {
    profile_id: String,
    source: PathBuf,
    expected_sha256: Option<String>,
}

impl DriverInstallRequest {
    /// 创建本地 JAR 安装请求。
    #[must_use]
    pub fn new(profile_id: impl Into<String>, source: impl Into<PathBuf>) -> Self {
        Self {
            profile_id: profile_id.into(),
            source: source.into(),
            expected_sha256: None,
        }
    }

    /// 设置调用方提供的 SHA-256；不匹配时拒绝安装。
    #[must_use]
    pub fn expected_sha256(mut self, expected_sha256: impl Into<String>) -> Self {
        self.expected_sha256 = Some(expected_sha256.into());
        self
    }

    pub(crate) fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub(crate) fn source(&self) -> &std::path::Path {
        &self.source
    }

    pub(crate) fn checksum(&self) -> Option<&str> {
        self.expected_sha256.as_deref()
    }
}
