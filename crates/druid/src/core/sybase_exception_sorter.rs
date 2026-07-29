//! `Sybase` 致命连接异常分类。

use super::{ExceptionSorter, ExceptionSorterProperties, SqlException};

/// `Sybase` 异常分类器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.SybaseExceptionSorter`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SybaseExceptionSorter;

impl ExceptionSorter for SybaseExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        if exception.is_recoverable() {
            return true;
        }
        exception.message().is_some_and(|message| {
            let error_text = message.to_uppercase();
            error_text.contains("JZ0C0") || error_text.contains("JZ0C1")
        })
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
