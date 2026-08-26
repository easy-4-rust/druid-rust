//! `SQLXML#getSource` 返回的 XML Source 平台句柄。

use crate::extensions::XmlSource;
use std::fmt;
use std::sync::Arc;

/// 不泄漏具体 XML 框架类型的 Source 句柄。
#[derive(Clone)]
pub struct RdbcXmlSource {
    value: Arc<dyn XmlSource>,
}

impl RdbcXmlSource {
    /// 包装一个 adapter XML Source。
    pub fn new(value: Arc<dyn XmlSource>) -> Self {
        Self { value }
    }

    /// 返回底层 Source representation。
    pub fn value(&self) -> &dyn XmlSource {
        self.value.as_ref()
    }
}

impl fmt::Debug for RdbcXmlSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcXmlSource")
            .field("value", &self.value)
            .finish()
    }
}

impl PartialEq for RdbcXmlSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
}

impl Eq for RdbcXmlSource {}
