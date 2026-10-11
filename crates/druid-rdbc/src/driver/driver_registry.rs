use crate::connection::Connection;
use crate::driver::{ConnectionProperties, DriverExtensionDescriptor};
use crate::error::RdbcError;
use std::sync::Arc;

use crate::driver::Driver;

/// 纯 standard provider registry。
///
/// Registry 只执行 JDBC URL negotiation；产品级 orchestration 留在上层 crate。
#[derive(Debug, Default)]
pub struct DriverRegistry {
    descriptors: Vec<DriverExtensionDescriptor>,
}

impl DriverRegistry {
    /// 创建空 registry。
    pub const fn new() -> Self {
        Self {
            descriptors: Vec::new(),
        }
    }

    /// 注册一个 standard provider descriptor。
    pub fn register(&mut self, descriptor: DriverExtensionDescriptor) {
        self.descriptors.push(descriptor);
    }

    /// 返回当前 provider descriptors。
    pub fn descriptors(&self) -> &[DriverExtensionDescriptor] {
        &self.descriptors
    }

    /// 按注册顺序返回首个接受 URL 的标准驱动；参数为连接 URL，未匹配时返回 `None`。
    #[must_use]
    pub fn first_accepted_driver(&self, url: &str) -> Option<Arc<dyn Driver>> {
        self.descriptors
            .iter()
            .find(|descriptor| descriptor.driver().accepts_url(url))
            .map(|descriptor| Arc::clone(descriptor.driver()))
    }

    /// 按注册顺序寻找第一个接受 URL 的 driver。
    pub async fn connect(
        &self,
        url: &str,
        properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        for descriptor in &self.descriptors {
            if !descriptor.driver().accepts_url(url) {
                continue;
            }
            if let Some(connection) = descriptor.driver().connect(url, properties).await? {
                return Ok(Some(connection));
            }
        }
        Ok(None)
    }
}
