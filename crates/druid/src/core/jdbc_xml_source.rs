//! `SQLXML#getSource` 返回的 XML Source 平台句柄。

use std::fmt;
use std::sync::Arc;

/// 物理 XML Source marker SPI。
pub trait PhysicalXmlSource: fmt::Debug + Send + Sync {}

/// 不泄漏具体 XML 框架类型的 Source 句柄。
#[derive(Clone)]
pub struct JdbcXmlSource {
    physical: Arc<dyn PhysicalXmlSource>,
}

impl JdbcXmlSource {
    /// 包装物理 XML Source。
    pub fn new(physical: Arc<dyn PhysicalXmlSource>) -> Self {
        Self { physical }
    }

    /// 返回物理 Source SPI。
    pub fn physical(&self) -> &dyn PhysicalXmlSource {
        self.physical.as_ref()
    }
}

impl fmt::Debug for JdbcXmlSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcXmlSource")
            .field("physical", &self.physical)
            .finish()
    }
}

impl PartialEq for JdbcXmlSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for JdbcXmlSource {}
