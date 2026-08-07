//! `SQLXML#getSource` 返回的 XML Source 平台句柄。

use std::fmt;
use std::sync::Arc;

/// 物理 XML Source marker SPI。
pub trait PhysicalXmlSource: fmt::Debug + Send + Sync {}

/// 不泄漏具体 XML 框架类型的 Source 句柄。
#[derive(Clone)]
pub struct RdbcXmlSource {
    physical: Arc<dyn PhysicalXmlSource>,
}

impl RdbcXmlSource {
    /// 包装物理 XML Source。
    pub fn new(physical: Arc<dyn PhysicalXmlSource>) -> Self {
        Self { physical }
    }

    /// 返回物理 Source SPI。
    pub fn physical(&self) -> &dyn PhysicalXmlSource {
        self.physical.as_ref()
    }
}

impl fmt::Debug for RdbcXmlSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcXmlSource")
            .field("physical", &self.physical)
            .finish()
    }
}

impl PartialEq for RdbcXmlSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcXmlSource {}
