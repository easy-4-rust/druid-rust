//! `DB2` 致命连接异常分类。

use super::{ExceptionSorter, ExceptionSorterProperties, SqlException};

/// `DB2` 异常分类器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.DB2ExceptionSorter`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Db2ExceptionSorter;

impl ExceptionSorter for Db2ExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        exception.is_recoverable()
            || exception
                .sql_state()
                .is_some_and(|sql_state| sql_state.starts_with("08"))
            || matches!(
                exception.error_code(),
                -512 | -514 | -516 | -518 | -525 | -909 | -918 | -924
            )
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
