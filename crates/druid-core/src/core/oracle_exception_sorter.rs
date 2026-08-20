//! Oracle 致命连接异常分类。
//!
//! 对应 Java：`com.alibaba.druid.pool.vendor.OracleExceptionSorter`。

use super::{
    AbstractOracleExceptionSorter, ExceptionSorter, ExceptionSorterProperties, SqlException,
    ORACLE_FATAL_ERROR_CODES_PROPERTY,
};
use std::collections::BTreeSet;

/// Oracle 异常分类器。
///
/// 保留 Java 的 recoverable、vendor code、TNS 区间、错误消息和自定义 fatal
/// code 判定顺序。Java 的抽象父类状态通过组合的
/// `AbstractOracleExceptionSorter` 承载。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleExceptionSorter {
    oracle: AbstractOracleExceptionSorter,
}

impl OracleExceptionSorter {
    /// 创建分类器，并读取与 Java system property 同名的进程环境配置。
    ///
    /// Java 构造器调用 `configFromProperties(System.getProperties())`；Rust 进程
    /// 没有 JVM system properties，使用同名环境变量作为宿主注入边界。
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

impl Default for OracleExceptionSorter {
    fn default() -> Self {
        Self::new()
    }
}

impl ExceptionSorter for OracleExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        if exception.is_recoverable() {
            return true;
        }

        // `wrapping_abs` 精确保留 Java `Math.abs(Integer.MIN_VALUE)` 的溢出结果。
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
            if !(20000..21000).contains(&error_code)
                && [
                    "SOCKET",
                    "套接字",
                    "CONNECTION HAS ALREADY BEEN CLOSED",
                    "BROKEN PIPE",
                    "管道已结束",
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
