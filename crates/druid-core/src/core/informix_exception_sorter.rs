//! `Informix` 致命连接异常分类。

use super::{ExceptionSorter, ExceptionSorterProperties, SqlException};

/// `Informix` 异常分类器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.InformixExceptionSorter`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct InformixExceptionSorter;

impl ExceptionSorter for InformixExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        exception.is_recoverable()
            || matches!(
                exception.error_code(),
                -710 | -79716
                    | -79730
                    | -79734
                    | -79735
                    | -79736
                    | -79756
                    | -79757
                    | -79758
                    | -79759
                    | -79760
                    | -79788
                    | -79811
                    | -79812
                    | -79836
                    | -79837
                    | -79879
            )
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
