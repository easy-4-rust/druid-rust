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
    /// 数字遵循固定 JDK 17 oracle 的 `Integer.parseInt`：接受 BMP 十进制数字，
    /// 保留 ASCII 正负号和 i32 边界，不接受补充平面数字或非十进制数值字符。
    pub fn config_from_properties(&mut self, properties: Option<&ExceptionSorterProperties>) {
        let Some(property) =
            properties.and_then(|properties| properties.get(ORACLE_FATAL_ERROR_CODES_PROPERTY))
        else {
            return;
        };

        for item in property.split(',').filter(|item| !item.is_empty()) {
            if let Some(error_code) = parse_java_integer(item) {
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

/// 按 Java 的 UTF-16 char 十进制解析规则读取有符号整数。
fn parse_java_integer(value: &str) -> Option<i32> {
    let (negative, digits) = if let Some(digits) = value.strip_prefix('-') {
        (true, digits)
    } else {
        (false, value.strip_prefix('+').unwrap_or(value))
    };
    if digits.is_empty() {
        return None;
    }

    // JDK 17 Character.digit(char, 10) 的 37 段 BMP 十进制数字，每段恰为 0..9。
    // 用 UTF-16 单元遍历，使补充平面数学数字与 Java 一样因代理项而被拒绝。
    const ZEROES: [u16; 37] = [
        0x0030, 0x0660, 0x06f0, 0x07c0, 0x0966, 0x09e6, 0x0a66, 0x0ae6, 0x0b66, 0x0be6, 0x0c66,
        0x0ce6, 0x0d66, 0x0de6, 0x0e50, 0x0ed0, 0x0f20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946,
        0x19d0, 0x1a80, 0x1a90, 0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0,
        0xa9f0, 0xaa50, 0xabf0, 0xff10,
    ];
    let limit = if negative { i32::MIN } else { -i32::MAX };
    let mut result = 0_i32;
    for unit in digits.encode_utf16() {
        let digit = ZEROES
            .iter()
            .find_map(|zero| unit.checked_sub(*zero).filter(|digit| *digit < 10))?;
        // 负数累积可完整表示 MIN_VALUE，正数结果则受 -MAX_VALUE 限制。
        result = result.checked_mul(10)?.checked_sub(i32::from(digit))?;
        if result < limit {
            return None;
        }
    }
    Some(if negative { result } else { -result })
}
