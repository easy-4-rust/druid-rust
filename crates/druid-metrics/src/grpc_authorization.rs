use std::fmt;

use base64::Engine as _;
use tonic::metadata::{Ascii, MetadataValue};

/// 生产 gRPC Connect request 使用的 Authorization metadata provider。
///
/// Rust-only 安全对象；metadata 只在 request boundary 注入，Debug 永不输出值。
#[derive(Clone, Eq, PartialEq)]
pub struct GrpcAuthorization {
    value: MetadataValue<Ascii>,
}

impl GrpcAuthorization {
    /// 从 Basic 用户名与密码创建标准 Authorization metadata。
    #[must_use]
    pub fn basic(username: &str, password: &str) -> Self {
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        let header = format!("Basic {encoded}");
        // `Basic ` 与 STANDARD base64 仅产生合法 ASCII metadata 字节。
        let mut value = MetadataValue::try_from(header)
            .expect("Basic plus standard base64 is valid ASCII metadata");
        value.set_sensitive(true);
        Self { value }
    }

    /// 把 redacted provider 持有的值注入一次 Tonic request metadata。
    pub(crate) fn apply<T>(&self, request: &mut tonic::Request<T>) {
        request
            .metadata_mut()
            .insert("authorization", self.value.clone());
    }
}

impl fmt::Debug for GrpcAuthorization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GrpcAuthorization")
            .field("value", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::GrpcAuthorization;

    #[test]
    fn applied_authorization_metadata_is_sensitive_and_debug_redacted() {
        const CANONICAL_AUTHORIZATION: &str = "Basic c2Vuc2l0aXZlLXVzZXI6c2Vuc2l0aXZlLXBhc3N3b3Jk";
        let authorization = GrpcAuthorization::basic("sensitive-user", "sensitive-password");

        for provider in [authorization.clone(), authorization] {
            let mut request = tonic::Request::new(());
            provider.apply(&mut request);
            let metadata = request
                .metadata()
                .get("authorization")
                .expect("authorization metadata is applied");
            assert!(
                metadata.is_sensitive(),
                "Authorization must use HPACK never-index and redacted Debug"
            );
            let debug = format!("request={request:?} metadata={:?}", request.metadata());
            assert!(!debug.contains(CANONICAL_AUTHORIZATION), "{debug}");
            assert!(!debug.contains("sensitive-password"), "{debug}");
        }
    }
}
