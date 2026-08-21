use std::net::SocketAddr;

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
}

impl AdminServerConfig {
    /// 使用显式地址和共享安全策略创建配置。
    #[must_use]
    pub const fn new(
        http_addr: SocketAddr,
        grpc_addr: SocketAddr,
        tls: Option<AdminTlsFiles>,
        auth: AdminAuthMode,
    ) -> Self {
        Self {
            http_addr,
            grpc_addr,
            tls,
            auth,
        }
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
        let config = Self::new(http_addr, grpc_addr, tls, auth);
        config.validate()?;
        Ok(config)
    }

    /// 校验监听暴露面与安全配置是否匹配。
    pub fn validate(&self) -> Result<(), AdminConfigError> {
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

    fn requires_remote_security(&self) -> bool {
        !self.http_addr.ip().is_loopback() || !self.grpc_addr.ip().is_loopback()
    }
}

impl Default for AdminServerConfig {
    fn default() -> Self {
        Self::new(
            SocketAddr::from(([127, 0, 0, 1], 8080)),
            SocketAddr::from(([127, 0, 0, 1], 9090)),
            None,
            AdminAuthMode::Disabled,
        )
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

fn optional_env(name: &str) -> Result<Option<String>, AdminConfigError> {
    match std::env::var(name) {
        Ok(value) if value.trim().is_empty() => Ok(None),
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(AdminConfigError::NonUnicodeEnvironment),
    }
}
