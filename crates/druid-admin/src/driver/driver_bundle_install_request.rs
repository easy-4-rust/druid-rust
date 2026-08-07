use super::DriverBundleFile;

/// 一个 JDBC 产品的显式多 JAR 安装请求。
#[derive(Debug, Clone)]
pub struct DriverBundleInstallRequest {
    profile_id: String,
    files: Vec<DriverBundleFile>,
}

impl DriverBundleInstallRequest {
    /// 创建 bundle 请求；文件顺序不影响内容版本身份。
    #[must_use]
    pub fn new(profile_id: impl Into<String>, files: Vec<DriverBundleFile>) -> Self {
        Self {
            profile_id: profile_id.into(),
            files,
        }
    }

    /// 返回目标数据库 profile ID。
    #[must_use]
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// 返回组成该 JDBC 驱动 bundle 的全部本地 JAR。
    #[must_use]
    pub fn files(&self) -> &[DriverBundleFile] {
        &self.files
    }
}
