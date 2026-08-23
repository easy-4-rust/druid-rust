//! `SQLXML#setResult` 返回的 XML Result 平台句柄。

use std::fmt;
use std::sync::Arc;

/// 物理 XML Result marker SPI。
pub trait PhysicalXmlResult: fmt::Debug + Send + Sync {}

/// 不泄漏具体 XML 框架类型的 Result 句柄。
#[derive(Clone)]
pub struct RdbcXmlResult {
    physical: Arc<dyn PhysicalXmlResult>,
}

impl RdbcXmlResult {
    /// 包装物理 XML Result。
    pub fn new(physical: Arc<dyn PhysicalXmlResult>) -> Self {
        Self { physical }
    }

    /// 返回物理 Result SPI。
    pub fn physical(&self) -> &dyn PhysicalXmlResult {
        self.physical.as_ref()
    }
}

impl fmt::Debug for RdbcXmlResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcXmlResult")
            .field("physical", &self.physical)
            .finish()
    }
}

impl PartialEq for RdbcXmlResult {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.physical, &other.physical)
    }
}

impl Eq for RdbcXmlResult {}
