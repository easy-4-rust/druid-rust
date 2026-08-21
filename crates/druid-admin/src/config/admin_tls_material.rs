use std::fmt;
use std::sync::Arc;

use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

use super::{AdminConfigError, AdminTlsFiles};

/// 已解析、可同时装配到 Axum listener 与 Tonic server 的 TLS 材料。
///
/// Rust-only 安全对象；私钥不会进入 Debug。
#[derive(Clone)]
pub struct AdminTlsMaterial {
    certificate_chain_pem: Arc<[u8]>,
    private_key_pem: Arc<[u8]>,
    http_server_config: Arc<rustls::ServerConfig>,
}

impl AdminTlsMaterial {
    /// 从成对 PEM 文件读取并验证证书链与私钥。
    pub(crate) fn load(files: &AdminTlsFiles) -> Result<Self, AdminConfigError> {
        let certificate_chain_pem = std::fs::read(files.certificate_chain_path())
            .map_err(AdminConfigError::TlsMaterialRead)?;
        let private_key_pem =
            std::fs::read(files.private_key_path()).map_err(AdminConfigError::TlsMaterialRead)?;
        let certificate_chain = CertificateDer::pem_slice_iter(&certificate_chain_pem)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| AdminConfigError::InvalidTlsMaterial)?;
        if certificate_chain.is_empty() {
            return Err(AdminConfigError::InvalidTlsMaterial);
        }
        let private_key = PrivateKeyDer::from_pem_slice(&private_key_pem)
            .map_err(|_| AdminConfigError::InvalidTlsMaterial)?;
        let http_server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certificate_chain, private_key)
            .map_err(|_| AdminConfigError::InvalidTlsMaterial)?;
        Ok(Self {
            certificate_chain_pem: certificate_chain_pem.into(),
            private_key_pem: private_key_pem.into(),
            http_server_config: Arc::new(http_server_config),
        })
    }

    /// 返回 Axum TLS listener 使用的 rustls server 配置。
    #[must_use]
    pub fn http_server_config(&self) -> Arc<rustls::ServerConfig> {
        Arc::clone(&self.http_server_config)
    }

    /// 返回 Tonic server 使用的同一证书链和私钥身份。
    #[must_use]
    pub fn grpc_identity(&self) -> tonic::transport::Identity {
        tonic::transport::Identity::from_pem(
            self.certificate_chain_pem.as_ref(),
            self.private_key_pem.as_ref(),
        )
    }
}

impl fmt::Debug for AdminTlsMaterial {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdminTlsMaterial")
            .finish_non_exhaustive()
    }
}
