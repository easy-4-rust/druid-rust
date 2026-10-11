use std::fmt;

use super::{AdminRole, BasicCredentialVerifier};

/// Admin 应用层认证模式。
///
/// Rust-only 安全对象；Java MonitorViewServlet 的 credentials/session 语义映射到
/// `Basic`；远程部署通过 `BasicRoles` 隔离操作员和指标采集身份。
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum AdminAuthMode {
    /// 仅允许 loopback 部署使用的无认证模式。
    Disabled,
    /// 复用 Java MonitorViewServlet 用户名/密码和 session 语义。
    Basic(BasicCredentialVerifier),
    /// 操作员与指标采集端使用不同的 Basic 凭据。
    BasicRoles {
        /// HTTP 管理操作员凭据。
        operator: BasicCredentialVerifier,
        /// gRPC 指标采集凭据。
        ingest: BasicCredentialVerifier,
    },
}

impl AdminAuthMode {
    /// 创建用户名/密码认证模式。
    #[must_use]
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Self {
        let username = username.into();
        let password = password.into();
        Self::Basic(BasicCredentialVerifier::new(&username, &password))
    }

    /// 创建独立的 HTTP 操作员和 gRPC 采集凭据。
    #[must_use]
    pub fn basic_with_ingest(
        operator_username: impl Into<String>,
        operator_password: impl Into<String>,
        ingest_username: impl Into<String>,
        ingest_password: impl Into<String>,
    ) -> Self {
        let operator_username = operator_username.into();
        let operator_password = operator_password.into();
        let ingest_username = ingest_username.into();
        let ingest_password = ingest_password.into();
        Self::BasicRoles {
            operator: BasicCredentialVerifier::new(&operator_username, &operator_password),
            ingest: BasicCredentialVerifier::new(&ingest_username, &ingest_password),
        }
    }

    /// 返回是否使用不同的操作员与采集身份。
    #[must_use]
    pub const fn has_separate_roles(&self) -> bool {
        matches!(self, Self::BasicRoles { .. })
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
            Self::BasicRoles { operator, ingest } => {
                operator.is_configuration_valid()
                    && ingest.is_configuration_valid()
                    && operator != ingest
            }
        }
    }

    /// 使用与 Basic header 相同的 digest truth 校验 Servlet login form。
    #[must_use]
    pub fn authorizes_credentials(&self, username: &str, password: &str) -> bool {
        match self {
            Self::Disabled => false,
            Self::Basic(verifier) => verifier.authorizes_credentials(username, password),
            Self::BasicRoles { operator, .. } => {
                operator.authorizes_credentials(username, password)
            }
        }
    }

    /// 校验 HTTP `Authorization` header；认证关闭时仅供 loopback 模式放行。
    #[must_use]
    pub fn authorizes_header(&self, authorization: Option<&str>) -> bool {
        self.authorizes_role_header(AdminRole::Operator, authorization)
    }

    /// 按请求角色校验 Basic header；角色凭据不能互换。
    #[must_use]
    pub fn authorizes_role_header(&self, role: AdminRole, authorization: Option<&str>) -> bool {
        match self {
            Self::Disabled => true,
            Self::Basic(verifier) => {
                authorization.is_some_and(|value| verifier.authorizes_header(value))
            }
            Self::BasicRoles { operator, ingest } => {
                authorization.is_some_and(|value| match role {
                    AdminRole::Operator => operator.authorizes_header(value),
                    AdminRole::Ingest => ingest.authorizes_header(value),
                })
            }
        }
    }

    /// 在 Tonic transport/service 边界校验标准 `authorization` metadata。
    pub fn authorize_grpc_request(
        &self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let authorization = request
            .metadata()
            .get("authorization")
            .and_then(|value| value.to_str().ok());
        if self.authorizes_role_header(AdminRole::Ingest, authorization) {
            request.extensions_mut().insert(AdminRole::Ingest);
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
            Self::BasicRoles { .. } => formatter
                .debug_struct("BasicRoles")
                .field("operator", &"[REDACTED]")
                .field("ingest", &"[REDACTED]")
                .finish(),
        }
    }
}
