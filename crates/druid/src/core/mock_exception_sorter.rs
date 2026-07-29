//! Druid Mock 驱动致命连接异常分类。
//!
//! 对应 Java：`com.alibaba.druid.pool.vendor.MockExceptionSorter`。

use super::{ExceptionSorter, ExceptionSorterProperties, SqlException};

const MOCK_CONNECTION_CLOSED_EXCEPTION: &str =
    "com.alibaba.druid.mock.MockConnectionClosedException";

/// Mock 驱动异常分类器。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MockExceptionSorter;

impl MockExceptionSorter {
    /// 返回进程级共享实例。
    ///
    /// 对应 Java：`MockExceptionSorter#getInstance()`。
    pub fn get_instance() -> &'static Self {
        static INSTANCE: MockExceptionSorter = MockExceptionSorter;
        &INSTANCE
    }
}

impl ExceptionSorter for MockExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        exception.is_instance_of(MOCK_CONNECTION_CLOSED_EXCEPTION)
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
