//! `SQLXML#setResult` 返回的 XML Result 平台句柄。

use crate::extensions::XmlResult;
use std::fmt;
use std::sync::Arc;

/// 不泄漏具体 XML 框架类型的 Result 句柄。
#[derive(Clone)]
pub struct RdbcXmlResult {
    value: Arc<dyn XmlResult>,
}

impl RdbcXmlResult {
    /// 包装一个 adapter XML Result。
    pub fn new(value: Arc<dyn XmlResult>) -> Self {
        Self { value }
    }

    /// 返回底层 Result representation。
    pub fn value(&self) -> &dyn XmlResult {
        self.value.as_ref()
    }
}

impl fmt::Debug for RdbcXmlResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcXmlResult")
            .field("value", &self.value)
            .finish()
    }
}

impl PartialEq for RdbcXmlResult {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}

impl Eq for RdbcXmlResult {}
