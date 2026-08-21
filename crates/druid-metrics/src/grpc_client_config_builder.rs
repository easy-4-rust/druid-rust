use std::time::Duration;

use tonic::transport::Endpoint;

use crate::grpc_client::{GrpcAuthorization, GrpcClientConfig, DEFAULT_ACK_WINDOW_CAPACITY};
use crate::MetricsError;

/// [`GrpcClientConfig`] 的验证 builder。
///
/// Rust-only 配置对象；ACK window 默认 256，build 在任何网络行为前执行
/// remote HTTPS/auth、identity、deadline 与容量校验。
pub struct GrpcClientConfigBuilder {
    endpoint: Endpoint,
    service_name: String,
    instance_id: String,
    boot_id: String,
    ack_window_capacity: usize,
    shutdown_timeout: Duration,
    authorization: Option<GrpcAuthorization>,
}

impl GrpcClientConfigBuilder {
    /// 使用必需 endpoint/stream identity 创建 builder。
    pub fn new(
        endpoint: Endpoint,
        service_name: impl Into<String>,
        instance_id: impl Into<String>,
        boot_id: impl Into<String>,
    ) -> Self {
        Self {
            endpoint,
            service_name: service_name.into(),
            instance_id: instance_id.into(),
            boot_id: boot_id.into(),
            ack_window_capacity: DEFAULT_ACK_WINDOW_CAPACITY,
            shutdown_timeout: Duration::from_secs(5),
            authorization: None,
        }
    }

    /// 设置 ACK window；0 会在 build 时拒绝。
    #[must_use]
    pub fn ack_window_capacity(mut self, capacity: usize) -> Self {
        self.ack_window_capacity = capacity;
        self
    }

    /// 设置 endpoint/RPC/Hello 与 shutdown 共用期限。
    #[must_use]
    pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    /// 设置初连与重连共用的 redacted Authorization provider。
    #[must_use]
    pub fn authorization(mut self, authorization: GrpcAuthorization) -> Self {
        self.authorization = Some(authorization);
        self
    }

    /// 构造并完整校验配置，不执行网络操作。
    pub fn build(self) -> Result<GrpcClientConfig, MetricsError> {
        let config = GrpcClientConfig {
            endpoint: self.endpoint,
            service_name: self.service_name,
            instance_id: self.instance_id,
            boot_id: self.boot_id,
            ack_window_capacity: self.ack_window_capacity,
            shutdown_timeout: self.shutdown_timeout,
            authorization: self.authorization,
        };
        config.validate()?;
        Ok(config)
    }
}
