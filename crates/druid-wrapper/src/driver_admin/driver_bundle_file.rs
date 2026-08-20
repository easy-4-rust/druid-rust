use std::path::{Path, PathBuf};

/// JDBC bundle 中一个本地 JAR 及其可选预期摘要。
#[derive(Debug, Clone)]
pub struct DriverBundleFile {
    path: PathBuf,
    expected_sha256: Option<String>,
}

impl DriverBundleFile {
    /// 创建本地 bundle 文件描述。
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            expected_sha256: None,
        }
    }

    /// 固定该 JAR 的预期 SHA-256。
    #[must_use]
    pub fn expected_sha256(mut self, expected: impl Into<String>) -> Self {
        self.expected_sha256 = Some(expected.into());
        self
    }

    /// 返回待安装 JAR 的本地路径。
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回调用方指定的预期 SHA-256；未固定摘要时返回 `None`。
    #[must_use]
    pub fn checksum(&self) -> Option<&str> {
        self.expected_sha256.as_deref()
    }
}
