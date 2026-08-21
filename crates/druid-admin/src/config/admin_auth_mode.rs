use std::fmt;

use super::BasicCredentialVerifier;

/// Admin 应用层认证模式。
///
/// Rust-only 安全对象；Java MonitorViewServlet 的 credentials/session 语义映射到
/// `Basic`，gRPC 在 service boundary 复用同一配置。
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum AdminAuthMode {
    /// 仅允许 loopback 部署使用的无认证模式。
    Disabled,
    /// 复用 Java MonitorViewServlet 用户名/密码和 session 语义。
    Basic(BasicCredentialVerifier),
}

impl AdminAuthMode {
    /// 创建用户名/密码认证模式。
    #[must_use]
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        let username = username.into();
        let password = password.into();
        Self::Basic(BasicCredentialVerifier::new(&username, &password))
    }

    /// 返回是否启用应用层认证。
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// 返回 auth 配置是否能形成可验证的 canonical credential。
    pub(crate) fn is_configuration_valid(&self) -> bool {
        match self {
            Self::Disabled => true,
            Self::Basic(verifier) => verifier.is_configuration_valid(),
        }
    }

    /// 使用与 Basic header 相同的 digest truth 校验 Servlet login form。
    #[must_use]
    pub fn authorizes_credentials(&self, username: &str, password: &str) -> bool {
        match self {
            Self::Disabled => false,
            Self::Basic(verifier) => verifier.authorizes_credentials(username, password),
        }
    }

    /// 校验 HTTP `Authorization` header；认证关闭时仅供 loopback 模式放行。
    #[must_use]
    pub fn authorizes_header(&self, authorization: Option<&str>) -> bool {
        match self {
            Self::Disabled => true,
            Self::Basic(verifier) => {
                authorization.is_some_and(|value| verifier.authorizes_header(value))
            }
        }
    }

    /// 在 Tonic transport/service 边界校验标准 `authorization` metadata。
    pub fn authorize_grpc_request(
        &self,
        request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let authorization = request
            .metadata()
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        if self.authorizes_header(authorization) {
            Ok(request)
        } else {
            Err(tonic::Status::unauthenticated("authentication required"))
        }
    }
}

impl fmt::Debug for AdminAuthMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => formatter.write_str("Disabled"),
            Self::Basic(_) => formatter
                .debug_struct("Basic")
                .field("username", &"[REDACTED]")
                .field("password", &"[REDACTED]")
                .finish(),
        }
    }
}
