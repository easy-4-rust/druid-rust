use crate::connection::Connection;
use crate::error::RdbcError;
use std::collections::BTreeMap;

/// Driver connection properties；有序映射保证 deterministic diagnostics。
pub type ConnectionProperties = BTreeMap<String, String>;

/// JDBC `DriverPropertyInfo` 的 immutable Rust descriptor。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverPropertyInfo {
    /// property name。
    pub name: String,
    /// optional configured value。
    pub value: Option<String>,
    /// 是否必填。
    pub required: bool,
    /// 可选 choices；`None` 与空集合保持不同。
    pub choices: Option<Vec<String>>,
}

/// JDBC `Driver` 的 URL negotiation 标准。
#[async_trait::async_trait]
pub trait Driver: Send + Sync + 'static {
    /// 返回 driver 是否理解给定 URL。
    fn accepts_url(&self, url: &str) -> bool;

    /// 尝试连接；不理解 URL 时返回 `Ok(None)`。
    async fn connect(
        &self,
        url: &str,
        properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError>;

    /// 返回 driver property descriptors。
    fn property_info(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Vec<DriverPropertyInfo>, RdbcError> {
        Ok(Vec::new())
    }

    /// 返回 major version。
    fn major_version(&self) -> u32 {
        0
    }

    /// 返回 minor version。
    fn minor_version(&self) -> u32 {
        0
    }
}
