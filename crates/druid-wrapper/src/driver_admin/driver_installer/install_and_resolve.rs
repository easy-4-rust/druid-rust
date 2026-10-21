use super::*;

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
}
