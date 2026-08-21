//! `PostgreSQL` 致命连接异常分类。

use super::{ExceptionSorter, ExceptionSorterProperties, SqlException};

/// `PostgreSQL` 异常分类器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.PGExceptionSorter`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PgExceptionSorter;

impl ExceptionSorter for PgExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        exception.is_recoverable()
            || exception
                .sql_state()
                .is_some_and(|sql_state| sql_state.starts_with("08"))
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
