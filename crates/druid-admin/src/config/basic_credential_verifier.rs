use std::fmt;

use base64::Engine as _;
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq as _;

/// HTTP Basic 与 login form 共用的固定长度凭据 verifier。
///
/// Rust-only 安全对象；只保存 canonical `username:password` 的 SHA-256 digest，
/// 不保存可被 Debug 或错误链意外输出的明文副本。
#[derive(Clone, Eq, PartialEq)]
pub struct BasicCredentialVerifier {
    credential_digest: [u8; 32],
    configuration_valid: bool,
}

impl BasicCredentialVerifier {
    /// 从配置用户名与密码创建 verifier。
    #[must_use]
    pub fn new(username: &str, password: &str) -> Self {
        Self {
            credential_digest: credential_digest(username, password),
            configuration_valid: !username.is_empty()
                && !username.contains(':')
                && !password.is_empty(),
        }
    }

    /// 返回配置 credentials 是否能形成严格 Basic canonical 值。
    #[must_use]
    pub const fn is_configuration_valid(&self) -> bool {
        self.configuration_valid
    }

    /// 严格解析并校验 RFC Basic Authorization 值。
    #[must_use]
    pub fn authorizes_header(&self, authorization: &str) -> bool {
        let Some((scheme, encoded)) = authorization.split_once(' ') else {
            return false;
        };
        if !scheme.eq_ignore_ascii_case("basic")
            || encoded.is_empty()
            || encoded.bytes().any(|byte| byte.is_ascii_whitespace())
        {
            return false;
        }
        let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded) else {
            return false;
        };
        let Ok(canonical) = std::str::from_utf8(&decoded) else {
            return false;
        };
        let Some((username, password)) = canonical.split_once(':') else {
            return false;
        };
        self.authorizes_credentials(username, password)
    }

    /// 使用与 Basic header 相同的 canonical digest truth 校验 login form。
    #[must_use]
    pub fn authorizes_credentials(&self, username: &str, password: &str) -> bool {
        if !self.configuration_valid
            || username.is_empty()
            || username.contains(':')
            || password.is_empty()
        {
            return false;
        }
        let candidate = credential_digest(username, password);
        bool::from(candidate.ct_eq(&self.credential_digest))
    }
}

impl fmt::Debug for BasicCredentialVerifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BasicCredentialVerifier")
            .field("credential_digest", &"[REDACTED]")
            .finish()
    }
}

fn credential_digest(username: &str, password: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"druid-admin-basic-v1\0");
    digest.update(username.as_bytes());
    digest.update(b":");
    digest.update(password.as_bytes());
    digest.finalize().into()
}
