use super::{DriverInstallRequest, DriverInstallerError, InstalledDriver};
use druid_wrapper::driver::{DatabaseProfileId, DriverRuntimeMode, DruidDriverRegistry};
use druid_wrapper::jdbc_agent::JdbcAgentOptions;
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 显式下载或导入 JDBC JAR 的内容寻址安装器。
#[derive(Debug, Clone)]
pub struct DriverInstaller {
    root: PathBuf,
    http_client: reqwest::Client,
}

impl DriverInstaller {
    /// 单个驱动工件最大允许 256 MiB。
    pub const MAX_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024;
    /// 单次显式远程安装的总超时。
    pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(2);

    /// 创建使用指定受管根目录的安装器。
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            http_client: reqwest::Client::new(),
        }
    }

    /// 使用调用方配置的 HTTPS client 创建安装器。
    #[must_use]
    pub fn with_http_client(root: impl Into<PathBuf>, http_client: reqwest::Client) -> Self {
        Self {
            root: root.into(),
            http_client,
        }
    }

    /// 返回默认安装根；优先使用 `DRUID_DRIVER_HOME`，否则落到当前目录。
    pub fn default_root() -> Result<PathBuf, DriverInstallerError> {
        Ok(std::env::var_os("DRUID_DRIVER_HOME").map_or_else(
            || {
                std::env::current_dir()
                    .map(|directory| directory.join(".druid-rust").join("drivers"))
            },
            |directory| Ok(PathBuf::from(directory)),
        )?)
    }

    /// 从本地 JAR 安装指定 JDBC 档案，不访问网络。
    pub async fn install_file(
        &self,
        request: &DriverInstallRequest,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        Self::require_jdbc_profile(request.profile_id())?;
        let source = request.source();
        let metadata = tokio::fs::metadata(source).await?;
        if !metadata.is_file() {
            return Err(DriverInstallerError::InvalidArtifact(
                source.display().to_string(),
            ));
        }
        Self::validate_size(metadata.len())?;
        let file_name = Self::jar_file_name(source)?;
        let bytes = tokio::fs::read(source).await?;
        let source = tokio::fs::canonicalize(source).await?.display().to_string();
        self.install_bytes(
            request.profile_id(),
            &file_name,
            bytes,
            request.checksum(),
            source,
        )
        .await
    }

    /// 通过调用方显式给出的 URL 和必填 SHA-256 下载 JDBC JAR。
    pub async fn install_url(
        &self,
        profile_id: &str,
        url: &str,
        file_name: &str,
        expected_sha256: &str,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        Self::require_jdbc_profile(profile_id)?;
        Self::validate_jar_name(file_name)?;
        Self::normalize_checksum(expected_sha256)?;
        let parsed_url = reqwest::Url::parse(url)
            .ok()
            .filter(|parsed_url| parsed_url.scheme() == "https")
            .ok_or_else(|| DriverInstallerError::InvalidUrl(url.to_owned()))?;
        let download = async {
            let mut response = self
                .http_client
                .get(parsed_url)
                .send()
                .await?
                .error_for_status()?;
            if response.url().scheme() != "https" {
                return Err(DriverInstallerError::InvalidUrl(response.url().to_string()));
            }
            if let Some(length) = response.content_length() {
                Self::validate_size(length)?;
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                let next_length = bytes.len().saturating_add(chunk.len());
                Self::validate_size(u64::try_from(next_length).unwrap_or(u64::MAX))?;
                bytes.extend_from_slice(&chunk);
            }
            Ok::<_, DriverInstallerError>(bytes)
        };
        let bytes = tokio::time::timeout(Self::DOWNLOAD_TIMEOUT, download)
            .await
            .map_err(|_| DriverInstallerError::DownloadTimeout(url.to_owned()))??;
        self.install_bytes(
            profile_id,
            file_name,
            bytes,
            Some(expected_sha256),
            url.to_owned(),
        )
        .await
    }

    /// 安装 Druid 自有 Agent uber-jar；驱动 JAR 仍按产品分别管理。
    pub async fn install_agent_file(
        &self,
        source: impl AsRef<Path>,
        expected_sha256: Option<&str>,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        let source = source.as_ref();
        let metadata = tokio::fs::metadata(source).await?;
        if !metadata.is_file() {
            return Err(DriverInstallerError::InvalidArtifact(
                source.display().to_string(),
            ));
        }
        Self::validate_size(metadata.len())?;
        let bytes = tokio::fs::read(source).await?;
        let source_label = tokio::fs::canonicalize(source).await?.display().to_string();
        self.install_bytes(
            "jdbc-agent",
            "druid-jdbc-agent.jar",
            bytes,
            expected_sha256,
            source_label,
        )
        .await
    }

    /// 返回某产品当前激活且校验未被篡改的安装记录。
    pub async fn active_installation(
        &self,
        profile_id: &str,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        if profile_id != "jdbc-agent" {
            Self::require_jdbc_profile(profile_id)?;
        }
        let activation_directory = self.profile_root(profile_id).join("activations");
        let mut entries = match tokio::fs::read_dir(&activation_directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(DriverInstallerError::NotInstalled(profile_id.to_owned()));
            }
            Err(error) => return Err(error.into()),
        };
        let mut active: Option<InstalledDriver> = None;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_file() {
                let bytes = tokio::fs::read(entry.path()).await?;
                let installation: InstalledDriver = serde_json::from_slice(&bytes)?;
                if installation.profile_id() == profile_id
                    && active.as_ref().is_none_or(|current| {
                        installation.installed_at_epoch_millis()
                            > current.installed_at_epoch_millis()
                    })
                {
                    active = Some(installation);
                }
            }
        }
        let active =
            active.ok_or_else(|| DriverInstallerError::NotInstalled(profile_id.to_owned()))?;
        let bytes = tokio::fs::read(active.path()).await?;
        let actual = Self::sha256(&bytes);
        if actual != active.sha256() {
            return Err(DriverInstallerError::ChecksumMismatch {
                expected: active.sha256().to_owned(),
                actual,
            });
        }
        Ok(active)
    }

    /// 基于已安装 Agent 与产品驱动生成不经 shell 的跨平台 Java 启动配置。
    pub async fn jdbc_agent_options(
        &self,
        profile_id: &str,
        java_program: impl Into<std::ffi::OsString>,
    ) -> Result<JdbcAgentOptions, DriverInstallerError> {
        let agent = self.active_installation("jdbc-agent").await?;
        let driver = self.active_installation(profile_id).await?;
        Ok(JdbcAgentOptions::java(
            java_program,
            vec![agent.path().to_owned(), driver.path().to_owned()],
        )?)
    }

    async fn install_bytes(
        &self,
        profile_id: &str,
        file_name: &str,
        bytes: Vec<u8>,
        expected_sha256: Option<&str>,
        source: String,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        Self::validate_jar_name(file_name)?;
        Self::validate_size(u64::try_from(bytes.len()).unwrap_or(u64::MAX))?;
        if !bytes.starts_with(b"PK") {
            return Err(DriverInstallerError::InvalidArtifact(format!(
                "{file_name} is not a ZIP/JAR archive"
            )));
        }
        let sha256 = Self::sha256(&bytes);
        if let Some(expected) = expected_sha256 {
            let expected = Self::normalize_checksum(expected)?;
            if expected != sha256 {
                return Err(DriverInstallerError::ChecksumMismatch {
                    expected,
                    actual: sha256,
                });
            }
        }

        let profile_root = self.profile_root(profile_id);
        let object_directory = profile_root.join("objects").join(&sha256);
        let artifact_path = object_directory.join(file_name);
        tokio::fs::create_dir_all(&object_directory).await?;
        if tokio::fs::metadata(&artifact_path).await.is_err() {
            let temporary = object_directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
            tokio::fs::write(&temporary, &bytes).await?;
            tokio::fs::rename(&temporary, &artifact_path).await?;
        }
        let artifact_path = tokio::fs::canonicalize(&artifact_path).await?;
        let installed_at_epoch_millis = chrono::Utc::now().timestamp_millis();
        let installation = InstalledDriver::new(
            profile_id.to_owned(),
            file_name.to_owned(),
            sha256,
            artifact_path,
            source,
            installed_at_epoch_millis,
        );
        let activation_directory = profile_root.join("activations");
        tokio::fs::create_dir_all(&activation_directory).await?;
        let activation_name = format!("{installed_at_epoch_millis}-{}.json", uuid::Uuid::new_v4());
        let activation_path = activation_directory.join(activation_name);
        let temporary = activation_directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&temporary, serde_json::to_vec_pretty(&installation)?).await?;
        tokio::fs::rename(temporary, activation_path).await?;
        Ok(installation)
    }

    fn require_jdbc_profile(profile_id: &str) -> Result<(), DriverInstallerError> {
        let profile_id = DatabaseProfileId::new(profile_id)?;
        let registry = DruidDriverRegistry::builtin()?;
        let profile = registry.profile(&profile_id)?;
        if profile.runtime_mode() != DriverRuntimeMode::JdbcAgent {
            return Err(DriverInstallerError::NotJdbcProfile(
                profile.id().to_string(),
            ));
        }
        Ok(())
    }

    fn profile_root(&self, profile_id: &str) -> PathBuf {
        self.root.join(profile_id)
    }

    fn jar_file_name(path: &Path) -> Result<String, DriverInstallerError> {
        let file_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| DriverInstallerError::InvalidArtifact(path.display().to_string()))?;
        Self::validate_jar_name(file_name)?;
        Ok(file_name.to_owned())
    }

    fn validate_jar_name(file_name: &str) -> Result<(), DriverInstallerError> {
        let path = Path::new(file_name);
        let valid = path.file_name().and_then(OsStr::to_str) == Some(file_name)
            && path
                .extension()
                .and_then(OsStr::to_str)
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"));
        if valid {
            Ok(())
        } else {
            Err(DriverInstallerError::InvalidArtifact(file_name.to_owned()))
        }
    }

    fn validate_size(length: u64) -> Result<(), DriverInstallerError> {
        if length > Self::MAX_ARTIFACT_BYTES {
            Err(DriverInstallerError::ArtifactTooLarge(length))
        } else {
            Ok(())
        }
    }

    fn normalize_checksum(checksum: &str) -> Result<String, DriverInstallerError> {
        let checksum = checksum.to_ascii_lowercase();
        if checksum.len() == 64 && checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Ok(checksum)
        } else {
            Err(DriverInstallerError::InvalidChecksum(checksum))
        }
    }

    fn sha256(bytes: &[u8]) -> String {
        let mut checksum = String::with_capacity(64);
        for byte in Sha256::digest(bytes) {
            write!(&mut checksum, "{byte:02x}").expect("writing into String cannot fail");
        }
        checksum
    }
}
