//! 永不判定为致命异常的 sorter。

use super::{ExceptionSorter, ExceptionSorterProperties, SqlException};

/// 空异常分类器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.NullExceptionSorter`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NullExceptionSorter;

static INSTANCE: NullExceptionSorter = NullExceptionSorter;

impl NullExceptionSorter {
    /// 返回进程内共享实例。
    ///
    /// 对应 Java: `NullExceptionSorter#getInstance()`。
    pub fn get_instance() -> &'static Self {
        &INSTANCE
    }
}

impl ExceptionSorter for NullExceptionSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        false
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
