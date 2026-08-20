//! `OceanBase Oracle` 模式致命连接异常分类。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.vendor.OceanBaseOracleExceptionSorter`。

use super::{
    AbstractOracleExceptionSorter, ExceptionSorter, ExceptionSorterProperties, SqlException,
    ORACLE_FATAL_ERROR_CODES_PROPERTY,
};
use std::collections::BTreeSet;

/// `OceanBase Oracle` 模式异常分类器。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OceanBaseOracleExceptionSorter {
    oracle: AbstractOracleExceptionSorter,
}

impl OceanBaseOracleExceptionSorter {
    /// 创建分类器，并从同名环境变量读取 Java system property 的宿主映射。
    pub fn new() -> Self {
        let mut sorter = Self {
            oracle: AbstractOracleExceptionSorter::new(),
        };
        if let Ok(property) = std::env::var(ORACLE_FATAL_ERROR_CODES_PROPERTY) {
            let properties = ExceptionSorterProperties::from([(
                ORACLE_FATAL_ERROR_CODES_PROPERTY.to_string(),
                property,
            )]);
            sorter.config_from_properties(Some(&properties));
        }
        sorter
    }

    /// 返回自定义致命错误码集合。
    pub fn fatal_error_codes(&self) -> &BTreeSet<i32> {
        self.oracle.fatal_error_codes()
    }

    /// 替换整个自定义致命错误码集合。
    pub fn set_fatal_error_codes(&mut self, fatal_error_codes: BTreeSet<i32>) {
        self.oracle.set_fatal_error_codes(fatal_error_codes);
    }
}

impl Default for OceanBaseOracleExceptionSorter {
    fn default() -> Self {
        Self::new()
    }
}

impl ExceptionSorter for OceanBaseOracleExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        if exception.is_recoverable()
            || exception
                .sql_state()
                .is_some_and(|sql_state| sql_state.starts_with("08"))
        {
            return true;
        }

        let error_code = exception.error_code().wrapping_abs();
        if matches!(
            error_code,
            28 | 600
                | 1012
                | 1014
                | 1033
                | 1034
                | 1035
                | 1089
                | 1090
                | 1092
                | 1094
                | 2396
                | 3106
                | 3111
                | 3113
                | 3114
                | 3134
                | 3135
                | 3136
                | 3138
                | 3142
                | 3143
                | 3144
                | 3145
                | 3149
                | 6801
                | 6802
                | 6805
                | 9918
                | 9920
                | 9921
                | 17001
                | 17002
                | 17008
                | 17009
                | 17024
                | 17089
                | 17401
                | 17409
                | 17410
                | 17416
                | 17438
                | 17442
                | 25407
                | 25408
                | 25409
                | 25425
                | 29276
                | 30676
        ) || (12100..=12299).contains(&error_code)
        {
            return true;
        }

        if let Some(message) = exception.message() {
            let error_text = message.to_uppercase();
            if (!(20000..21000).contains(&error_code)
                && [
                    "SOCKET",
                    "套接字",
                    "CONNECTION HAS ALREADY BEEN CLOSED",
                    "BROKEN PIPE",
                    "管道已结束",
                ]
                .iter()
                .any(|fragment| error_text.contains(fragment)))
                || [
                    "COMMUNICATIONS LINK FAILURE",
                    "COULD NOT CREATE CONNECTION",
                    "ACCESS DENIED FOR USER",
                    "NO DATASOURCE",
                    "NO ALIVE DATASOURCE",
                ]
                .iter()
                .any(|fragment| error_text.contains(fragment))
            {
                return true;
            }
        }

        self.oracle.contains(error_code)
    }

    fn config_from_properties(&mut self, properties: Option<&ExceptionSorterProperties>) {
        self.oracle.config_from_properties(properties);
    }
}
