/// Admin 安全配置错误；错误文本不会回显凭据或密码。
///
/// Rust-only 错误对象；Java Admin 没有对应类型。
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AdminConfigError {
    /// 地址环境变量不是有效的 SocketAddr。
    #[error("admin bind address is invalid")]
    InvalidAddress(#[source] std::net::AddrParseError),
    /// 环境变量包含非 Unicode 数据。
    #[error("admin security environment contains non-Unicode data")]
    NonUnicodeEnvironment,
    /// TLS 证书链与私钥必须成对配置。
    #[error("TLS certificate chain and private key must be configured together")]
    IncompleteTlsConfiguration,
    /// 配置了不受支持的认证模式。
    #[error("unsupported admin authentication mode")]
    UnsupportedAuthMode,
    /// basic 模式缺少用户名或密码。
    #[error("basic admin authentication requires non-empty username and password")]
    MissingBasicCredentials,
    /// Basic credentials 不能形成严格 canonical 值。
    #[error("basic admin authentication credentials are invalid")]
    InvalidBasicCredentials,
    /// Shutdown deadline 必须是非零毫秒数。
    #[error("admin shutdown timeout must be a non-zero millisecond value")]
    InvalidShutdownTimeout,
    /// 非 loopback 暴露缺少 TLS 或应用层认证。
    #[error("non-loopback admin listeners require both TLS and authentication")]
    InsecureRemoteBind,
    /// 无法读取 TLS 文件。
    #[error("failed to read admin TLS material")]
    TlsMaterialRead(#[source] std::io::Error),
    /// PEM 内容不是有效的证书链与私钥组合。
    #[error("admin TLS certificate chain or private key is invalid")]
    InvalidTlsMaterial,
}
