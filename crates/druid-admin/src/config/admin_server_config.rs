use std::net::SocketAddr;
use std::time::Duration;

use super::{AdminAuthMode, AdminConfigError, AdminTlsFiles, AdminTlsMaterial};

/// Standalone Admin 两个监听器及其共享安全策略。
///
/// Rust-only 配置对象；Java Admin 没有对应类型。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminServerConfig {
    http_addr: SocketAddr,
    grpc_addr: SocketAddr,
    tls: Option<AdminTlsFiles>,
    auth: AdminAuthMode,
    shutdown_timeout: Duration,
}

impl AdminServerConfig {
    /// 使用显式地址和共享安全策略创建并验证配置。
    pub fn try_new(
        http_addr: SocketAddr,
        grpc_addr: SocketAddr,
        tls: Option<AdminTlsFiles>,
        auth: AdminAuthMode,
    ) -> Result<Self, AdminConfigError> {
        Self::try_new_with_shutdown_timeout(
            http_addr,
            grpc_addr,
            tls,
            auth,
            default_shutdown_timeout(),
        )
    }

    /// 从 standalone binary 环境变量读取并校验配置。
    pub fn from_env() -> Result<Self, AdminConfigError> {
        let http_addr = parse_address("DRUID_ADMIN_HTTP_ADDR", "127.0.0.1:8080")?;
        let grpc_addr = parse_address("DRUID_ADMIN_GRPC_ADDR", "127.0.0.1:9090")?;
        let certificate_chain_path = optional_env("DRUID_ADMIN_TLS_CERT_PATH")?;
        let private_key_path = optional_env("DRUID_ADMIN_TLS_KEY_PATH")?;
        let tls = match (certificate_chain_path, private_key_path) {
            (None, None) => None,
            (Some(certificate_chain_path), Some(private_key_path)) => {
                Some(AdminTlsFiles::new(certificate_chain_path, private_key_path))
            }
            _ => return Err(AdminConfigError::IncompleteTlsConfiguration),
        };
        let auth = match optional_env("DRUID_ADMIN_AUTH_MODE")?.as_deref() {
            None | Some("disabled") => AdminAuthMode::Disabled,
            Some("basic") => AdminAuthMode::basic(
                required_env("DRUID_ADMIN_LOGIN_USERNAME")?,
                required_env("DRUID_ADMIN_LOGIN_PASSWORD")?,
            ),
            Some(_) => return Err(AdminConfigError::UnsupportedAuthMode),
        };
        let shutdown_timeout = parse_shutdown_timeout()?;
        Self::try_new_with_shutdown_timeout(http_addr, grpc_addr, tls, auth, shutdown_timeout)
    }

    /// 校验监听暴露面与安全配置是否匹配。
    fn validate(&self) -> Result<(), AdminConfigError> {
        if self.shutdown_timeout.is_zero() {
            return Err(AdminConfigError::InvalidShutdownTimeout);
        }
        if !self.auth.is_configuration_valid() {
            return Err(AdminConfigError::InvalidBasicCredentials);
        }
        if self.requires_remote_security() && (self.tls.is_none() || !self.auth.is_enabled()) {
            return Err(AdminConfigError::InsecureRemoteBind);
        }
        Ok(())
    }

    /// 读取并解析 HTTP/gRPC 共用的 PEM 证书链与私钥。
    pub fn load_tls_material(&self) -> Result<Option<AdminTlsMaterial>, AdminConfigError> {
        self.tls.as_ref().map(AdminTlsMaterial::load).transpose()
    }

    /// 返回 HTTP 监听地址。
    #[must_use]
    pub const fn http_addr(&self) -> SocketAddr {
        self.http_addr
    }

    /// 返回 gRPC 监听地址。
    #[must_use]
    pub const fn grpc_addr(&self) -> SocketAddr {
        self.grpc_addr
    }

    /// 返回 HTTP 与 gRPC 共享的 TLS 文件配置。
    #[must_use]
    pub const fn tls(&self) -> Option<&AdminTlsFiles> {
        self.tls.as_ref()
    }

    /// 返回 HTTP 与 gRPC 共享的应用层认证模式。
    #[must_use]
    pub const fn auth(&self) -> &AdminAuthMode {
        &self.auth
    }

    /// 返回进程收到终止信号后等待 owned server handles 的总期限。
    #[must_use]
    pub const fn shutdown_timeout(&self) -> Duration {
        self.shutdown_timeout
    }

    fn requires_remote_security(&self) -> bool {
        !self.http_addr.ip().is_loopback() || !self.grpc_addr.ip().is_loopback()
    }
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self {
            http_addr: SocketAddr::from(([127, 0, 0, 1], 8080)),
            grpc_addr: SocketAddr::from(([127, 0, 0, 1], 9090)),
            tls: None,
            auth: AdminAuthMode::Disabled,
            shutdown_timeout: default_shutdown_timeout(),
        }
    }
}

impl AdminServerConfig {
    fn try_new_with_shutdown_timeout(
        http_addr: SocketAddr,
        grpc_addr: SocketAddr,
        tls: Option<AdminTlsFiles>,
        auth: AdminAuthMode,
        shutdown_timeout: Duration,
    ) -> Result<Self, AdminConfigError> {
        let config = Self {
            http_addr,
            grpc_addr,
            tls,
            auth,
            shutdown_timeout,
        };
        config.validate()?;
        Ok(config)
    }
}

fn parse_address(name: &str, default: &str) -> Result<SocketAddr, AdminConfigError> {
    optional_env(name)?
        .as_deref()
        .unwrap_or(default)
        .parse()
        .map_err(AdminConfigError::InvalidAddress)
}

fn required_env(name: &str) -> Result<String, AdminConfigError> {
    optional_env(name)?.ok_or(AdminConfigError::MissingBasicCredentials)
}

fn parse_shutdown_timeout() -> Result<Duration, AdminConfigError> {
    let Some(value) = optional_env("DRUID_ADMIN_SHUTDOWN_TIMEOUT_MS")? else {
        return Ok(default_shutdown_timeout());
    };
    let milliseconds = value
        .parse::<u64>()
        .map_err(|_| AdminConfigError::InvalidShutdownTimeout)?;
    let timeout = Duration::from_millis(milliseconds);
    if timeout.is_zero() {
        return Err(AdminConfigError::InvalidShutdownTimeout);
    }
    Ok(timeout)
}

const fn default_shutdown_timeout() -> Duration {
    Duration::from_secs(10)
}

fn optional_env(name: &str) -> Result<Option<String>, AdminConfigError> {
    match std::env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(AdminConfigError::NonUnicodeEnvironment),
    }
}
