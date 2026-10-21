use super::*;

impl DriverInstaller {
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

    pub(crate) async fn active_installation_with_lease(
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

    pub(crate) async fn active_installation_unlocked(
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

    pub(crate) async fn installation_records(
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

    pub(crate) async fn verify_record(
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

    pub(crate) async fn write_activation(
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

    pub(crate) async fn install_bytes(
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

    pub(crate) fn require_jdbc_profile(profile_id: &str) -> Result<(), DriverInstallerError> {
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

    pub(crate) fn driver_class(profile_id: &str) -> Result<Option<String>, DriverInstallerError> {
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

    pub(crate) fn profile_root(&self, profile_id: &str) -> PathBuf {
        self.root.join(profile_id)
    }

    pub(crate) fn java_program(java_home: &Path) -> PathBuf {
        #[cfg(target_os = "windows")]
        let executable = "java.exe";
        #[cfg(not(target_os = "windows"))]
        let executable = "java";
        java_home.join("bin").join(executable)
    }

    pub(crate) async fn probe_java_version(
        java_program: &OsStr,
    ) -> Result<u16, DriverInstallerError> {
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

    pub(crate) async fn acquire_profile_lock(
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

    pub(crate) async fn acquire_usage_lease(
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

    pub(crate) async fn acquire_unused_version_lock(
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

    pub(crate) async fn usage_lock_path(
        &self,
        profile_id: &str,
        sha256: &str,
    ) -> Result<PathBuf, DriverInstallerError> {
        let directory = self.profile_root(profile_id).join("usage-locks");
        Self::reject_symlink_if_present(&directory).await?;
        tokio::fs::create_dir_all(&directory).await?;
        Ok(directory.join(format!("{sha256}.lock")))
    }

    pub(crate) async fn reject_symlink_if_present(path: &Path) -> Result<(), DriverInstallerError> {
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

    pub(crate) fn jar_file_name(path: &Path) -> Result<String, DriverInstallerError> {
        let file_name = path
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| DriverInstallerError::InvalidArtifact(path.display().to_string()))?;
        Self::validate_jar_name(file_name)?;
        Ok(file_name.to_owned())
    }

    pub(crate) fn validate_jar_name(file_name: &str) -> Result<(), DriverInstallerError> {
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

    pub(crate) fn validate_size(length: u64) -> Result<(), DriverInstallerError> {
        if length > Self::MAX_ARTIFACT_BYTES {
            Err(DriverInstallerError::ArtifactTooLarge(length))
        } else {
            Ok(())
        }
    }

    pub(crate) fn normalize_checksum(checksum: &str) -> Result<String, DriverInstallerError> {
        let checksum = checksum.to_ascii_lowercase();
        if checksum.len() == 64 && checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Ok(checksum)
        } else {
            Err(DriverInstallerError::InvalidChecksum(checksum))
        }
    }

    pub(crate) fn sha256(bytes: &[u8]) -> String {
        Self::hex_digest(Sha256::digest(bytes))
    }

    pub(crate) fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
        let mut checksum = String::with_capacity(64);
        for byte in bytes.as_ref() {
            write!(&mut checksum, "{byte:02x}").expect("writing into String cannot fail");
        }
        checksum
    }
}
