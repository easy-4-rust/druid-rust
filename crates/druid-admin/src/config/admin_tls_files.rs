use std::path::{Path, PathBuf};

/// Admin HTTP 与 gRPC 共享的 TLS 文件位置。
///
/// Rust-only 配置对象；Java Admin 没有对应类型。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminTlsFiles {
    certificate_chain_path: PathBuf,
    private_key_path: PathBuf,
}

impl AdminTlsFiles {
    /// 创建证书链与私钥文件配置。
    #[must_use]
    pub fn new(
        certificate_chain_path: impl Into<PathBuf>,
        private_key_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            certificate_chain_path: certificate_chain_path.into(),
            private_key_path: private_key_path.into(),
        }
    }

    /// 返回 PEM 证书链路径。
    #[must_use]
    pub fn certificate_chain_path(&self) -> &Path {
        &self.certificate_chain_path
    }

    /// 返回 PEM 私钥路径。
    #[must_use]
    pub fn private_key_path(&self) -> &Path {
        &self.private_key_path
    }
}
