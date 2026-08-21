use std::fmt;

use base64::Engine as _;
use subtle::ConstantTimeEq as _;

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
    Basic { username: String, password: String },
}

impl AdminAuthMode {
    /// 创建用户名/密码认证模式。
    #[must_use]
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self::Basic {
            username: username.into(),
            password: password.into(),
        }
    }

    /// 返回是否启用应用层认证。
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        !matches!(self, Self::Disabled)
    }

    /// 返回 Servlet login 表单校验使用的 credentials；禁用时返回 None。
    pub(crate) fn credentials(&self) -> Option<(&str, &str)> {
        match self {
            Self::Disabled => None,
            Self::Basic { username, password } => Some((username, password)),
        }
    }

    /// 校验 HTTP `Authorization` header；认证关闭时仅供 loopback 模式放行。
    #[must_use]
    pub fn authorizes_header(&self, authorization: Option<&str>) -> bool {
        let Self::Basic { username, password } = self else {
            return true;
        };
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        let expected = format!("Basic {encoded}");
        authorization.is_some_and(|actual| bool::from(actual.as_bytes().ct_eq(expected.as_bytes())))
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
            Self::Basic { username, .. } => formatter
                .debug_struct("Basic")
                .field("username", username)
                .field("password", &"[REDACTED]")
                .finish(),
        }
    }
}
