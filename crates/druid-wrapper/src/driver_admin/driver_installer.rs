use super::{
    DriverBundleInstallRequest, DriverInstallRequest, DriverInstallerError, InstalledDriver,
    JavaRuntimeInstallation,
};
use crate::driver::{DatabaseProfileId, DriverRuntimeMode, DruidDriverRegistry};
use crate::jdbc_agent::JdbcAgentOptions;
use fs2::FileExt;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fmt::Write;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

/// 显式下载或导入 JDBC JAR 的内容寻址安装器。
#[derive(Debug, Clone)]
pub struct DriverInstaller {
    root: PathBuf,
    http_client: reqwest::Client,
    offline: bool,
}

impl DriverInstaller {
    /// 单个驱动工件最大允许 256 MiB。
    pub const MAX_ARTIFACT_BYTES: u64 = 256 * 1024 * 1024;
    /// 单次显式远程安装的总超时。
    pub const DOWNLOAD_TIMEOUT: Duration = Duration::from_mins(2);
    /// 并发安装等待同一产品文件锁的最长时间。
    pub const LOCK_TIMEOUT: Duration = Duration::from_secs(30);

    /// 创建使用指定受管根目录的安装器。
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            http_client: reqwest::Client::new(),
            offline: false,
        }
    }

    /// 使用调用方配置的 HTTPS client 创建安装器。
    #[must_use]
    pub fn with_http_client(root: impl Into<PathBuf>, http_client: reqwest::Client) -> Self {
        Self {
            root: root.into(),
            http_client,
            offline: false,
        }
    }

    /// 禁止任何网络下载，仅允许校验和导入本地预装制品。
    #[must_use]
    pub const fn offline(mut self, offline: bool) -> Self {
        self.offline = offline;
        self
    }

    /// 返回默认安装根；显式环境变量优先，否则使用平台应用数据目录。
    pub fn default_root() -> Result<PathBuf, DriverInstallerError> {
        if let Some(directory) = std::env::var_os("DRUID_DRIVER_HOME") {
            return Ok(PathBuf::from(directory));
        }
        #[cfg(target_os = "windows")]
        if let Some(directory) = std::env::var_os("LOCALAPPDATA") {
            return Ok(PathBuf::from(directory).join("druid-rust").join("drivers"));
        }
        #[cfg(target_os = "macos")]
        if let Some(directory) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(directory)
                .join("Library")
                .join("Application Support")
                .join("druid-rust")
                .join("drivers"));
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if let Some(directory) = std::env::var_os("XDG_DATA_HOME") {
                return Ok(PathBuf::from(directory).join("druid-rust").join("drivers"));
            }
            if let Some(directory) = std::env::var_os("HOME") {
                return Ok(PathBuf::from(directory)
                    .join(".local")
                    .join("share")
                    .join("druid-rust")
                    .join("drivers"));
            }
        }
        Ok(std::env::current_dir()?.join(".druid-rust").join("drivers"))
    }

    /// 从本地 JAR 安装指定 JDBC 档案，不访问网络。
    pub async fn install_file(
        &self,
        request: &DriverInstallRequest,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        Self::require_jdbc_profile(request.profile_id())?;
        let source = request.source();
        let metadata = tokio::fs::symlink_metadata(source).await?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
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

    /// 从本地显式安装一个包含主驱动及传递依赖的 JDBC 多 JAR bundle。
    ///
    /// 每个 JAR 独立校验摘要，bundle 版本由排序后的文件名与摘要共同计算；
    /// 核心建池路径不会解析 Maven 或访问网络。
    pub async fn install_bundle_files(
        &self,
        request: &DriverBundleInstallRequest,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        Self::require_jdbc_profile(request.profile_id())?;
        if request.files().is_empty() {
            return Err(DriverInstallerError::InvalidArtifact(
                "JDBC bundle must contain at least one JAR".to_owned(),
            ));
        }
        let mut files = BTreeMap::<String, (Vec<u8>, String, String)>::new();
        let mut total_bytes = 0_u64;
        for file in request.files() {
            let metadata = tokio::fs::symlink_metadata(file.path()).await?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(DriverInstallerError::InvalidArtifact(
                    file.path().display().to_string(),
                ));
            }
            Self::validate_size(metadata.len())?;
            total_bytes = total_bytes.saturating_add(metadata.len());
            Self::validate_size(total_bytes)?;
            let file_name = Self::jar_file_name(file.path())?;
            if files.contains_key(&file_name) {
                return Err(DriverInstallerError::InvalidArtifact(format!(
                    "duplicate JDBC bundle file name: {file_name}"
                )));
            }
            let bytes = tokio::fs::read(file.path()).await?;
            if !bytes.starts_with(b"PK") {
                return Err(DriverInstallerError::InvalidArtifact(format!(
                    "{file_name} is not a ZIP/JAR archive"
                )));
            }
            let actual = Self::sha256(&bytes);
            if let Some(expected) = file.checksum() {
                let expected = Self::normalize_checksum(expected)?;
                if expected != actual {
                    return Err(DriverInstallerError::ChecksumMismatch { expected, actual });
                }
            }
            let source = tokio::fs::canonicalize(file.path())
                .await?
                .display()
                .to_string();
            files.insert(file_name, (bytes, actual, source));
        }

        let mut identity = Sha256::new();
        let mut jar_sha256 = BTreeMap::new();
        for (file_name, (_, sha256, _)) in &files {
            identity.update(file_name.as_bytes());
            identity.update([0]);
            identity.update(sha256.as_bytes());
            identity.update([0]);
            jar_sha256.insert(file_name.clone(), sha256.clone());
        }
        let bundle_sha256 = Self::hex_digest(identity.finalize());
        let _lock = self.acquire_profile_lock(request.profile_id()).await?;
        let profile_root = self.profile_root(request.profile_id());
        Self::reject_symlink_if_present(&profile_root).await?;
        let objects_directory = profile_root.join("objects");
        Self::reject_symlink_if_present(&objects_directory).await?;
        let object_directory = objects_directory.join(&bundle_sha256);
        Self::reject_symlink_if_present(&object_directory).await?;
        tokio::fs::create_dir_all(&object_directory).await?;
        for (file_name, (bytes, _, _)) in &files {
            let target = object_directory.join(file_name);
            if tokio::fs::metadata(&target).await.is_err() {
                let temporary = object_directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
                tokio::fs::write(&temporary, bytes).await?;
                tokio::fs::rename(temporary, target).await?;
            }
        }
        let primary_name = files.keys().next().cloned().expect("bundle is non-empty");
        let primary_path = tokio::fs::canonicalize(object_directory.join(&primary_name)).await?;
        let sources = files
            .values()
            .map(|(_, _, source)| source)
            .collect::<Vec<_>>();
        let installation = InstalledDriver::new_bundle(
            request.profile_id().to_owned(),
            primary_name,
            bundle_sha256,
            primary_path,
            serde_json::to_string(&sources)?,
            "NOASSERTION".to_owned(),
            Self::driver_class(request.profile_id())?,
            jar_sha256,
            chrono::Utc::now().timestamp_millis(),
        );
        let installation = self
            .verify_record(request.profile_id(), installation)
            .await?;
        self.write_activation(&installation).await
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
        if self.offline {
            return Err(DriverInstallerError::OfflineMode(url.to_owned()));
        }
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
        let metadata = tokio::fs::symlink_metadata(source).await?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
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
        let _lock = self.acquire_profile_lock(profile_id).await?;
        self.active_installation_unlocked(profile_id).await
    }

    /// 列出某产品可回滚的全部内容寻址版本，并按安装时间降序去重。
    pub async fn installations(
        &self,
        profile_id: &str,
    ) -> Result<Vec<InstalledDriver>, DriverInstallerError> {
        if profile_id != "jdbc-agent" {
            Self::require_jdbc_profile(profile_id)?;
        }
        let _lock = self.acquire_profile_lock(profile_id).await?;
        let mut versions = HashMap::<String, InstalledDriver>::new();
        for record in self.installation_records(profile_id).await? {
            versions
                .entry(record.sha256().to_owned())
                .and_modify(|current| {
                    if record.installed_at_epoch_millis() > current.installed_at_epoch_millis() {
                        *current = record.clone();
                    }
                })
                .or_insert(record);
        }
        let mut versions = versions.into_values().collect::<Vec<_>>();
        versions.sort_by_key(|version| std::cmp::Reverse(version.installed_at_epoch_millis()));
        Ok(versions)
    }

    /// 重新读取指定版本并校验路径边界与 SHA-256。
    pub async fn verify_installation(
        &self,
        profile_id: &str,
        sha256: &str,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        if profile_id != "jdbc-agent" {
            Self::require_jdbc_profile(profile_id)?;
        }
        let sha256 = Self::normalize_checksum(sha256)?;
        let _lock = self.acquire_profile_lock(profile_id).await?;
        let record = self
            .installation_records(profile_id)
            .await?
            .into_iter()
            .filter(|record| record.sha256() == sha256)
            .max_by_key(InstalledDriver::installed_at_epoch_millis)
            .ok_or_else(|| DriverInstallerError::ArtifactVersionNotFound {
                profile_id: profile_id.to_owned(),
                sha256: sha256.clone(),
            })?;
        self.verify_record(profile_id, record).await
    }

    /// 将已校验的旧版本重新写入激活日志，实现不改对象内容的原子回滚。
    pub async fn activate_version(
        &self,
        profile_id: &str,
        sha256: &str,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        if profile_id != "jdbc-agent" {
            Self::require_jdbc_profile(profile_id)?;
        }
        let sha256 = Self::normalize_checksum(sha256)?;
        let _lock = self.acquire_profile_lock(profile_id).await?;
        let record = self
            .installation_records(profile_id)
            .await?
            .into_iter()
            .find(|record| record.sha256() == sha256)
            .ok_or_else(|| DriverInstallerError::ArtifactVersionNotFound {
                profile_id: profile_id.to_owned(),
                sha256: sha256.clone(),
            })?;
        let record = self.verify_record(profile_id, record).await?;
        self.write_activation(&record).await?;
        self.active_installation_unlocked(profile_id).await
    }

    /// 删除非激活版本；激活版本必须先切换，避免正在使用的版本被移除。
    pub async fn remove_version(
        &self,
        profile_id: &str,
        sha256: &str,
    ) -> Result<(), DriverInstallerError> {
        if profile_id != "jdbc-agent" {
            Self::require_jdbc_profile(profile_id)?;
        }
        let sha256 = Self::normalize_checksum(sha256)?;
        let _lock = self.acquire_profile_lock(profile_id).await?;
        if self
            .active_installation_unlocked(profile_id)
            .await
            .is_ok_and(|active| active.sha256() == sha256)
        {
            return Err(DriverInstallerError::ActiveArtifact {
                profile_id: profile_id.to_owned(),
                sha256,
            });
        }
        let _usage_lock = self
            .acquire_unused_version_lock(profile_id, &sha256)
            .await?;
        let activation_directory = self.profile_root(profile_id).join("activations");
        let mut entries = tokio::fs::read_dir(&activation_directory).await?;
        let mut found = false;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_file() {
                continue;
            }
            let record: InstalledDriver =
                serde_json::from_slice(&tokio::fs::read(entry.path()).await?)?;
            if record.profile_id() == profile_id && record.sha256() == sha256 {
                tokio::fs::remove_file(entry.path()).await?;
                found = true;
            }
        }
        if !found {
            return Err(DriverInstallerError::ArtifactVersionNotFound {
                profile_id: profile_id.to_owned(),
                sha256,
            });
        }
        let object_directory = self.profile_root(profile_id).join("objects").join(&sha256);
        if tokio::fs::metadata(&object_directory).await.is_ok() {
            tokio::fs::remove_dir_all(object_directory).await?;
        }
        Ok(())
    }

    /// 基于已安装 Agent 与产品驱动生成不经 shell 的跨平台 Java 启动配置。
    pub async fn jdbc_agent_options(
        &self,
        profile_id: &str,
        java_program: impl Into<std::ffi::OsString>,
    ) -> Result<JdbcAgentOptions, DriverInstallerError> {
        let (agent, agent_lease) = self.active_installation_with_lease("jdbc-agent").await?;
        let (driver, driver_lease) = self.active_installation_with_lease(profile_id).await?;
        let java_program = java_program.into();
        let java_version = Self::probe_java_version(java_program.as_os_str()).await?;
        if java_version < 17 {
            return Err(DriverInstallerError::UnsupportedJavaVersion(java_version));
        }
        let jvm_options_hash = Self::sha256(java_program.to_string_lossy().as_bytes());
        let class_path = agent
            .class_path()
            .into_iter()
            .chain(driver.class_path())
            .collect::<Vec<_>>();
        Ok(JdbcAgentOptions::java(java_program, class_path)?
            .runtime_identity(
                format!("jdbc-agent:{}", agent.sha256()),
                format!("agent={};driver={}", agent.sha256(), driver.sha256()),
                jvm_options_hash,
            )
            .artifact_lease(agent_lease)
            .artifact_lease(driver_lease))
    }

    /// 登记并激活调用方已解压的 Java 17+ runtime；不会下载或执行 shell。
    pub async fn register_java_runtime(
        &self,
        java_home: impl AsRef<Path>,
        expected_sha256: &str,
    ) -> Result<JavaRuntimeInstallation, DriverInstallerError> {
        let expected_sha256 = Self::normalize_checksum(expected_sha256)?;
        let java_home = java_home.as_ref();
        let home_metadata = tokio::fs::symlink_metadata(java_home).await?;
        if home_metadata.file_type().is_symlink() || !home_metadata.is_dir() {
            return Err(DriverInstallerError::InvalidArtifact(
                java_home.display().to_string(),
            ));
        }
        let java_program = Self::java_program(java_home);
        let program_metadata = tokio::fs::symlink_metadata(&java_program).await?;
        if program_metadata.file_type().is_symlink() || !program_metadata.is_file() {
            return Err(DriverInstallerError::InvalidArtifact(
                java_program.display().to_string(),
            ));
        }
        let java_home = tokio::fs::canonicalize(java_home).await?;
        let java_program = tokio::fs::canonicalize(java_program).await?;
        if !java_program.starts_with(&java_home) {
            return Err(DriverInstallerError::InvalidArtifact(
                java_program.display().to_string(),
            ));
        }
        let actual_sha256 = Self::sha256(&tokio::fs::read(&java_program).await?);
        if actual_sha256 != expected_sha256 {
            return Err(DriverInstallerError::ChecksumMismatch {
                expected: expected_sha256,
                actual: actual_sha256,
            });
        }
        let major_version = Self::probe_java_version(java_program.as_os_str()).await?;
        if major_version < 17 {
            return Err(DriverInstallerError::UnsupportedJavaVersion(major_version));
        }
        let _lock = self.acquire_profile_lock("java-runtime").await?;
        let installation = JavaRuntimeInstallation::new(
            java_home.clone(),
            java_program,
            actual_sha256,
            major_version,
            java_home.display().to_string(),
            chrono::Utc::now().timestamp_millis(),
        );
        let directory = self.root.join("java-runtime").join("activations");
        Self::reject_symlink_if_present(&directory).await?;
        tokio::fs::create_dir_all(&directory).await?;
        let path = directory.join(format!(
            "{}-{}.json",
            installation.installed_at_epoch_millis(),
            uuid::Uuid::new_v4()
        ));
        let temporary = directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&temporary, serde_json::to_vec_pretty(&installation)?).await?;
        tokio::fs::rename(temporary, path).await?;
        Ok(installation)
    }

    /// 返回当前激活且摘要、路径和 Java 主版本均重新校验通过的 runtime。
    pub async fn active_java_runtime(
        &self,
    ) -> Result<JavaRuntimeInstallation, DriverInstallerError> {
        let _lock = self.acquire_profile_lock("java-runtime").await?;
        let directory = self.root.join("java-runtime").join("activations");
        Self::reject_symlink_if_present(&directory).await?;
        let mut entries = match tokio::fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(DriverInstallerError::JavaRuntimeNotInstalled);
            }
            Err(error) => return Err(error.into()),
        };
        let mut active = None;
        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if file_type.is_symlink() {
                return Err(DriverInstallerError::InvalidArtifact(
                    entry.path().display().to_string(),
                ));
            }
            if file_type.is_file() {
                let runtime: JavaRuntimeInstallation =
                    serde_json::from_slice(&tokio::fs::read(entry.path()).await?)?;
                if active
                    .as_ref()
                    .is_none_or(|current: &JavaRuntimeInstallation| {
                        runtime.installed_at_epoch_millis() > current.installed_at_epoch_millis()
                    })
                {
                    active = Some(runtime);
                }
            }
        }
        let active = active.ok_or(DriverInstallerError::JavaRuntimeNotInstalled)?;
        let metadata = tokio::fs::symlink_metadata(active.java_program()).await?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(DriverInstallerError::InvalidArtifact(
                active.java_program().display().to_string(),
            ));
        }
        let actual = Self::sha256(&tokio::fs::read(active.java_program()).await?);
        if actual != active.sha256() {
            return Err(DriverInstallerError::ChecksumMismatch {
                expected: active.sha256().to_owned(),
                actual,
            });
        }
        let major_version = Self::probe_java_version(active.java_program().as_os_str()).await?;
        if major_version < 17 {
            return Err(DriverInstallerError::UnsupportedJavaVersion(major_version));
        }
        Ok(active)
    }

    /// 使用已登记的受管 Java runtime 构造固定制品启动配置。
    pub async fn jdbc_agent_options_with_managed_java(
        &self,
        profile_id: &str,
    ) -> Result<JdbcAgentOptions, DriverInstallerError> {
        let runtime = self.active_java_runtime().await?;
        self.jdbc_agent_options(profile_id, runtime.java_program().as_os_str())
            .await
    }

    async fn active_installation_with_lease(
        &self,
        profile_id: &str,
    ) -> Result<(InstalledDriver, File), DriverInstallerError> {
        if profile_id != "jdbc-agent" {
            Self::require_jdbc_profile(profile_id)?;
        }
        let _lock = self.acquire_profile_lock(profile_id).await?;
        let active = self.active_installation_unlocked(profile_id).await?;
        let lease = self
            .acquire_usage_lease(profile_id, active.sha256())
            .await?;
        Ok((active, lease))
    }

    async fn active_installation_unlocked(
        &self,
        profile_id: &str,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        let active = self
            .installation_records(profile_id)
            .await?
            .into_iter()
            .max_by_key(InstalledDriver::installed_at_epoch_millis)
            .ok_or_else(|| DriverInstallerError::NotInstalled(profile_id.to_owned()))?;
        self.verify_record(profile_id, active).await
    }

    async fn installation_records(
        &self,
        profile_id: &str,
    ) -> Result<Vec<InstalledDriver>, DriverInstallerError> {
        let activation_directory = self.profile_root(profile_id).join("activations");
        Self::reject_symlink_if_present(&activation_directory).await?;
        let mut entries = match tokio::fs::read_dir(&activation_directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut records = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if file_type.is_symlink() {
                return Err(DriverInstallerError::InvalidArtifact(format!(
                    "activation metadata must not be a symbolic link: {}",
                    entry.path().display()
                )));
            }
            if file_type.is_file() {
                let record: InstalledDriver =
                    serde_json::from_slice(&tokio::fs::read(entry.path()).await?)?;
                if record.profile_id() == profile_id {
                    records.push(record);
                }
            }
        }
        Ok(records)
    }

    async fn verify_record(
        &self,
        profile_id: &str,
        record: InstalledDriver,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        let profile_root = tokio::fs::canonicalize(self.profile_root(profile_id)).await?;
        let expected = if record.jar_sha256().is_empty() {
            BTreeMap::from([(
                record
                    .path()
                    .file_name()
                    .and_then(OsStr::to_str)
                    .ok_or_else(|| {
                        DriverInstallerError::InvalidArtifact(record.path().display().to_string())
                    })?
                    .to_owned(),
                record.sha256().to_owned(),
            )])
        } else {
            record.jar_sha256().clone()
        };
        let directory = record.path().parent().ok_or_else(|| {
            DriverInstallerError::InvalidArtifact(record.path().display().to_string())
        })?;
        let mut identity = Sha256::new();
        for (file_name, expected_sha256) in &expected {
            Self::validate_jar_name(file_name)?;
            let path = directory.join(file_name);
            let metadata = tokio::fs::symlink_metadata(&path).await?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(DriverInstallerError::InvalidArtifact(
                    path.display().to_string(),
                ));
            }
            let artifact_path = tokio::fs::canonicalize(&path).await?;
            if !artifact_path.starts_with(&profile_root) {
                return Err(DriverInstallerError::InvalidArtifact(format!(
                    "installed artifact escapes managed profile root: {}",
                    path.display()
                )));
            }
            let actual = Self::sha256(&tokio::fs::read(&artifact_path).await?);
            if &actual != expected_sha256 {
                return Err(DriverInstallerError::ChecksumMismatch {
                    expected: expected_sha256.clone(),
                    actual,
                });
            }
            identity.update(file_name.as_bytes());
            identity.update([0]);
            identity.update(actual.as_bytes());
            identity.update([0]);
        }
        let actual_version = if record.is_bundle() {
            Self::hex_digest(identity.finalize())
        } else {
            expected.values().next().cloned().unwrap_or_default()
        };
        if actual_version != record.sha256() {
            return Err(DriverInstallerError::ChecksumMismatch {
                expected: record.sha256().to_owned(),
                actual: actual_version,
            });
        }
        Ok(record)
    }

    async fn write_activation(
        &self,
        installation: &InstalledDriver,
    ) -> Result<InstalledDriver, DriverInstallerError> {
        let last_activation = self
            .installation_records(installation.profile_id())
            .await?
            .into_iter()
            .map(|record| record.installed_at_epoch_millis())
            .max()
            .unwrap_or(i64::MIN);
        let activated = installation.reactivated(
            chrono::Utc::now()
                .timestamp_millis()
                .max(last_activation.saturating_add(1)),
        );
        let activation_directory = self
            .profile_root(installation.profile_id())
            .join("activations");
        Self::reject_symlink_if_present(&activation_directory).await?;
        tokio::fs::create_dir_all(&activation_directory).await?;
        let activation_name = format!(
            "{}-{}.json",
            activated.installed_at_epoch_millis(),
            uuid::Uuid::new_v4()
        );
        let activation_path = activation_directory.join(activation_name);
        let temporary = activation_directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
        tokio::fs::write(&temporary, serde_json::to_vec_pretty(&activated)?).await?;
        tokio::fs::rename(temporary, activation_path).await?;
        Ok(activated)
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

        let _lock = self.acquire_profile_lock(profile_id).await?;

        let profile_root = self.profile_root(profile_id);
        Self::reject_symlink_if_present(&profile_root).await?;
        let objects_directory = profile_root.join("objects");
        Self::reject_symlink_if_present(&objects_directory).await?;
        let object_directory = objects_directory.join(&sha256);
        Self::reject_symlink_if_present(&object_directory).await?;
        let artifact_path = object_directory.join(file_name);
        tokio::fs::create_dir_all(&object_directory).await?;
        if tokio::fs::metadata(&artifact_path).await.is_err() {
            let temporary = object_directory.join(format!(".tmp-{}", uuid::Uuid::new_v4()));
            tokio::fs::write(&temporary, &bytes).await?;
            tokio::fs::rename(&temporary, &artifact_path).await?;
        }
        let artifact_path = tokio::fs::canonicalize(&artifact_path).await?;
        let installation = InstalledDriver::new(
            profile_id.to_owned(),
            file_name.to_owned(),
            sha256,
            artifact_path,
            source,
            if profile_id == "jdbc-agent" {
                "Apache-2.0".to_owned()
            } else {
                "NOASSERTION".to_owned()
            },
            Self::driver_class(profile_id)?,
            chrono::Utc::now().timestamp_millis(),
        );
        let installation = self.verify_record(profile_id, installation).await?;
        self.write_activation(&installation).await
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

    fn driver_class(profile_id: &str) -> Result<Option<String>, DriverInstallerError> {
        if profile_id == "jdbc-agent" {
            return Ok(Some(
                "io.github.easy4rust.druid.agent.JdbcAgentMain".to_owned(),
            ));
        }
        let profile_id = DatabaseProfileId::new(profile_id)?;
        let registry = DruidDriverRegistry::builtin()?;
        Ok(registry
            .profile(&profile_id)?
            .driver_class()
            .map(ToOwned::to_owned))
    }

    fn profile_root(&self, profile_id: &str) -> PathBuf {
        self.root.join(profile_id)
    }

    fn java_program(java_home: &Path) -> PathBuf {
        #[cfg(target_os = "windows")]
        let executable = "java.exe";
        #[cfg(not(target_os = "windows"))]
        let executable = "java";
        java_home.join("bin").join(executable)
    }

    async fn probe_java_version(java_program: &OsStr) -> Result<u16, DriverInstallerError> {
        let mut command = Command::new(java_program);
        command.arg("-version").kill_on_drop(true);
        let output = tokio::time::timeout(Duration::from_secs(5), command.output())
            .await
            .map_err(|_| {
                DriverInstallerError::DownloadTimeout(format!(
                    "Java version probe: {}",
                    java_program.to_string_lossy()
                ))
            })??;
        if !output.status.success() {
            return Err(DriverInstallerError::InvalidArtifact(format!(
                "Java version probe failed: {}",
                java_program.to_string_lossy()
            )));
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let text = if stderr.trim().is_empty() {
            stdout.as_ref()
        } else {
            stderr.as_ref()
        };
        let version = text.split('"').nth(1).ok_or_else(|| {
            DriverInstallerError::InvalidArtifact(format!(
                "unrecognized Java version output: {text}"
            ))
        })?;
        let mut parts = version.split('.');
        let first = parts
            .next()
            .and_then(|value| value.parse::<u16>().ok())
            .ok_or_else(|| {
                DriverInstallerError::InvalidArtifact(format!(
                    "unrecognized Java version: {version}"
                ))
            })?;
        if first == 1 {
            parts
                .next()
                .and_then(|value| value.parse::<u16>().ok())
                .ok_or_else(|| {
                    DriverInstallerError::InvalidArtifact(format!(
                        "unrecognized Java version: {version}"
                    ))
                })
        } else {
            Ok(first)
        }
    }

    async fn acquire_profile_lock(
        &self,
        profile_id: &str,
    ) -> Result<ArtifactLock, DriverInstallerError> {
        tokio::fs::create_dir_all(&self.root).await?;
        Self::reject_symlink_if_present(&self.root).await?;
        let lock_directory = self.root.join(".locks");
        tokio::fs::create_dir_all(&lock_directory).await?;
        Self::reject_symlink_if_present(&lock_directory).await?;
        let lock_path = lock_directory.join(format!("{profile_id}.lock"));
        let display = lock_path.display().to_string();
        let timeout = Self::LOCK_TIMEOUT;
        let file = tokio::task::spawn_blocking(move || {
            let file = File::options()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&lock_path)?;
            let started = std::time::Instant::now();
            loop {
                match file.try_lock_exclusive() {
                    Ok(()) => return Ok(file),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if started.elapsed() >= timeout {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::TimedOut,
                                display.clone(),
                            ));
                        }
                        std::thread::sleep(Duration::from_millis(25));
                    }
                    Err(error) => return Err(error),
                }
            }
        })
        .await
        .map_err(|error| {
            DriverInstallerError::Io(std::io::Error::other(format!(
                "artifact lock task failed: {error}"
            )))
        })?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::TimedOut {
                DriverInstallerError::LockTimeout(error.to_string())
            } else {
                DriverInstallerError::Io(error)
            }
        })?;
        Ok(ArtifactLock(Arc::new(file)))
    }

    async fn acquire_usage_lease(
        &self,
        profile_id: &str,
        sha256: &str,
    ) -> Result<File, DriverInstallerError> {
        let path = self.usage_lock_path(profile_id, sha256).await?;
        tokio::task::spawn_blocking(move || {
            let file = File::options()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path)?;
            FileExt::lock_shared(&file)?;
            Ok::<File, std::io::Error>(file)
        })
        .await
        .map_err(|error| {
            DriverInstallerError::Io(std::io::Error::other(format!(
                "artifact usage lock task failed: {error}"
            )))
        })?
        .map_err(Into::into)
    }

    async fn acquire_unused_version_lock(
        &self,
        profile_id: &str,
        sha256: &str,
    ) -> Result<ArtifactLock, DriverInstallerError> {
        let path = self.usage_lock_path(profile_id, sha256).await?;
        let profile_id = profile_id.to_owned();
        let sha256 = sha256.to_owned();
        let file = tokio::task::spawn_blocking(move || {
            let file = File::options()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(path)?;
            match file.try_lock_exclusive() {
                Ok(()) => Ok(file),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    Err(DriverInstallerError::ActiveArtifact { profile_id, sha256 })
                }
                Err(error) => Err(error.into()),
            }
        })
        .await
        .map_err(|error| {
            DriverInstallerError::Io(std::io::Error::other(format!(
                "artifact usage check task failed: {error}"
            )))
        })??;
        Ok(ArtifactLock(Arc::new(file)))
    }

    async fn usage_lock_path(
        &self,
        profile_id: &str,
        sha256: &str,
    ) -> Result<PathBuf, DriverInstallerError> {
        let directory = self.profile_root(profile_id).join("usage-locks");
        Self::reject_symlink_if_present(&directory).await?;
        tokio::fs::create_dir_all(&directory).await?;
        Ok(directory.join(format!("{sha256}.lock")))
    }

    async fn reject_symlink_if_present(path: &Path) -> Result<(), DriverInstallerError> {
        match tokio::fs::symlink_metadata(path).await {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(DriverInstallerError::InvalidArtifact(format!(
                    "managed path must not be a symbolic link: {}",
                    path.display()
                )))
            }
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
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
        Self::hex_digest(Sha256::digest(bytes))
    }

    fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
        let mut checksum = String::with_capacity(64);
        for byte in bytes.as_ref() {
            write!(&mut checksum, "{byte:02x}").expect("writing into String cannot fail");
        }
        checksum
    }
}

/// 持有跨进程文件锁直到当前安装或校验操作结束。
struct ArtifactLock(Arc<File>);

impl Drop for ArtifactLock {
    fn drop(&mut self) {
        if Arc::strong_count(&self.0) == 1 {
            let _ = FileExt::unlock(self.0.as_ref());
        }
    }
}
