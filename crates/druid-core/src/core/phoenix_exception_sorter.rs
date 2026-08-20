//! Apache Phoenix 致命连接异常分类。
//!
//! 对应 Java：`com.alibaba.druid.pool.vendor.PhoenixExceptionSorter`。

use super::{ExceptionSorter, ExceptionSorterProperties, SqlException};

/// Phoenix 异常分类器。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PhoenixExceptionSorter;

impl ExceptionSorter for PhoenixExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        exception
            .message()
            .is_some_and(|message| message.contains("Connection is null or closed"))
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
