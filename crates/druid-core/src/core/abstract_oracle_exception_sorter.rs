//! Oracle sorter 共享的自定义致命错误码状态。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.vendor.AbstractOracleExceptionSorter`。

use super::ExceptionSorterProperties;
use std::collections::BTreeSet;

/// Java 配置项 `druid.oracle.fatalErrorCodes`。
pub const ORACLE_FATAL_ERROR_CODES_PROPERTY: &str = "druid.oracle.fatalErrorCodes";

/// Oracle 异常分类器共享状态。
///
/// Java 通过抽象父类的 `protected Set<Integer>` 共享状态；Rust 使用组合保存同一
/// 集合语义，既不模拟继承，也不把两个具体 sorter 合并成一个对象。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AbstractOracleExceptionSorter {
    fatal_error_codes: BTreeSet<i32>,
}

impl AbstractOracleExceptionSorter {
    /// 创建空的自定义错误码集合。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从可空连接属性追加自定义致命错误码。
    ///
    /// 对应 Java：
    /// `AbstractOracleExceptionSorter#configFromProperties(Properties)`。
    /// 逗号分隔项不会 trim；空项和非法整数被忽略，合法项按集合语义去重并追加。
    pub fn config_from_properties(&mut self, properties: Option<&ExceptionSorterProperties>) {
        let Some(property) =
            properties.and_then(|properties| properties.get(ORACLE_FATAL_ERROR_CODES_PROPERTY))
        else {
            return;
        };

        for item in property.split(',').filter(|item| !item.is_empty()) {
            if let Ok(error_code) = item.parse::<i32>() {
                self.fatal_error_codes.insert(error_code);
            }
        }
    }

    /// 返回自定义致命错误码集合。
    pub fn fatal_error_codes(&self) -> &BTreeSet<i32> {
        &self.fatal_error_codes
    }

    /// 替换整个自定义致命错误码集合。
    ///
    /// 对应 Java：
    /// `OracleExceptionSorter#setFatalErrorCodes(Set<Integer>)` 及
    /// `OceanBaseOracleExceptionSorter` 同名方法。
    pub fn set_fatal_error_codes(&mut self, fatal_error_codes: BTreeSet<i32>) {
        self.fatal_error_codes = fatal_error_codes;
    }

    /// 判断绝对值化后的 vendor error code 是否为用户配置项。
    pub fn contains(&self, error_code: i32) -> bool {
        self.fatal_error_codes.contains(&error_code)
    }
}
