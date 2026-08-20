use super::DatabaseProfileIdError;

/// 驱动目录、解析和建池阶段的统一错误。
#[derive(Debug, thiserror::Error)]
pub enum DriverRegistryError {
    #[error("invalid driver manifest: {0}")]
    InvalidManifest(String),
    #[error(transparent)]
    InvalidProfileId(#[from] DatabaseProfileIdError),
    #[error("unknown database profile '{0}'")]
    UnknownProfile(String),
    #[error("database profile '{profile}' uses runtime mode '{runtime}' which is not installed")]
    UnsupportedRuntime { profile: String, runtime: String },
    #[error("database profile '{profile}' received incompatible URL '{url}'")]
    InvalidUrl { profile: String, url: String },
    #[error(transparent)]
    Pool(#[from] druid_core::core::DruidError),
}
