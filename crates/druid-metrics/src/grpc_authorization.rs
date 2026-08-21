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
        let value = MetadataValue::try_from(header)
            .expect("Basic plus standard base64 is valid ASCII metadata");
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
