/// 驱动安装和解析失败。
#[derive(Debug, thiserror::Error)]
pub enum DriverInstallerError {
    #[error(transparent)]
    Registry(#[from] crate::driver::DriverRegistryError),
    #[error(transparent)]
    ProfileId(#[from] crate::driver::DatabaseProfileIdError),
    #[error("profile '{0}' is not a JDBC Agent profile")]
    NotJdbcProfile(String),
    #[error("driver artifact must be a regular .jar file: {0}")]
    InvalidArtifact(String),
    #[error("remote driver URL must be valid HTTPS: {0}")]
    InvalidUrl(String),
    #[error("artifact is larger than the {0} byte installation limit")]
    ArtifactTooLarge(u64),
    #[error("invalid SHA-256 '{0}'")]
    InvalidChecksum(String),
    #[error("SHA-256 mismatch: expected {expected}, actual {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("no installed artifact is active for '{0}'")]
    NotInstalled(String),
    #[error("artifact version '{sha256}' is not installed for '{profile_id}'")]
    ArtifactVersionNotFound { profile_id: String, sha256: String },
    #[error("artifact version '{sha256}' is active for '{profile_id}' and cannot be removed")]
    ActiveArtifact { profile_id: String, sha256: String },
    #[error("no verified Java runtime is installed")]
    JavaRuntimeNotInstalled,
    #[error("Java 17 or newer is required; detected Java {0}")]
    UnsupportedJavaVersion(u16),
    #[error("download failed: {0}")]
    Download(#[from] reqwest::Error),
    #[error("driver download exceeded the administrative timeout: {0}")]
    DownloadTimeout(String),
    #[error("network installation is disabled in offline mode: {0}")]
    OfflineMode(String),
    #[error("timed out waiting for artifact installation lock: {0}")]
    LockTimeout(String),
    #[error("invalid Java classpath: {0}")]
    ClassPath(#[from] std::env::JoinPathsError),
    #[error("filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("installation metadata is invalid: {0}")]
    Metadata(#[from] serde_json::Error),
}
