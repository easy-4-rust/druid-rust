//! Druid lexer 的 UTF-16 字符分类。

use crate::core::JavaString;

use super::LayoutCharacters;

/// SQL lexer 字符分类工具。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.CharTypes`。所有参数都是 Java
/// `char` 对应的 UTF-16 code unit，而不是 Rust Unicode scalar `char`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharTypes;

impl CharTypes {
    /// 判断 code unit 是否为 ASCII 十六进制数字。
    #[must_use]
    pub const fn is_hex(code_unit: u16) -> bool {
        (code_unit >= b'0' as u16 && code_unit <= b'9' as u16)
            || (code_unit >= b'A' as u16 && code_unit <= b'F' as u16)
            || (code_unit >= b'a' as u16 && code_unit <= b'f' as u16)
    }

    /// 判断 code unit 是否为 ASCII 十进制数字。
    #[must_use]
    pub const fn is_digit(code_unit: u16) -> bool {
        code_unit >= b'0' as u16 && code_unit <= b'9' as u16
    }

    /// 判断 code unit 是否为 ASCII 字母或下划线。
    ///
    /// 对应 Java：`letterOrUnderScore(char)`。
    #[must_use]
    pub const fn letter_or_under_score(code_unit: u16) -> bool {
        (code_unit >= b'A' as u16 && code_unit <= b'Z' as u16)
            || (code_unit >= b'a' as u16 && code_unit <= b'z' as u16)
            || code_unit == b'_' as u16
    }

    /// 判断 code unit 是否允许作为标识符首字符。
    ///
    /// Java 源码在 `code_unit == 256` 时因 `<= array.length` 的历史边界错误抛出
    /// `ArrayIndexOutOfBoundsException`；Rust 保留该可观察异常点为 panic。
    #[must_use]
    pub const fn is_first_identifier_char(code_unit: u16) -> bool {
        if code_unit < 256 {
            (code_unit >= b'A' as u16 && code_unit <= b'Z' as u16)
                || (code_unit >= b'a' as u16 && code_unit <= b'z' as u16)
                || (code_unit >= 0x00C0
                    && code_unit <= 0x00FF
                    && code_unit != 0x00D7
                    && code_unit != 0x00F7)
                || code_unit == b'`' as u16
                || code_unit == b'_' as u16
                || code_unit == b'$' as u16
        } else if code_unit == 256 {
            panic!("Java CharTypes firstIdentifierFlags index 256 out of bounds");
        } else {
            code_unit != 0x3000 && code_unit != 0xFF0C
        }
    }

    /// 判断 code unit 是否允许作为标识符后续字符。
    ///
    /// `code_unit == 256` 保留 Java 源码的历史数组越界异常。
    #[must_use]
    pub const fn is_identifier_char(code_unit: u16) -> bool {
        if code_unit < 256 {
            (code_unit >= b'A' as u16 && code_unit <= b'Z' as u16)
                || (code_unit >= b'a' as u16 && code_unit <= b'z' as u16)
                || (code_unit >= b'0' as u16 && code_unit <= b'9' as u16)
                || code_unit == b'_' as u16
                || code_unit == b'$' as u16
                || code_unit == b'#' as u16
        } else if code_unit == 256 {
            panic!("Java CharTypes identifierFlags index 256 out of bounds");
        } else {
            code_unit != 0x3000 && code_unit != 0xFF0C && code_unit != 0xFF09 && code_unit != 0xFF08
        }
    }

    /// 返回 Java 单字符缓存中的字符串。
    ///
    /// 只有 0..255 内且可作为标识符后续字符的 code unit 有缓存项。
    #[must_use]
    pub fn value_of(code_unit: u16) -> Option<JavaString> {
        if code_unit < 256 && Self::is_identifier_char(code_unit) {
            Some(JavaString::from_utf16(vec![code_unit]))
        } else {
            None
        }
    }

    /// 判断 Java code unit 是否为 Druid lexer 空白。
    ///
    /// EOI 明确不是空白；`code_unit == 256` 保留 Java 源码的历史数组越界异常。
    #[must_use]
    pub const fn is_whitespace(code_unit: u16) -> bool {
        if code_unit < 256 {
            ((code_unit <= 32 && code_unit != LayoutCharacters::EOI as u16)
                || (code_unit >= 0x7F && code_unit <= 0xA0))
                || code_unit == 0x3000
        } else if code_unit == 256 {
            panic!("Java CharTypes whitespaceFlags index 256 out of bounds");
        } else {
            code_unit == 0x3000
        }
    }

    /// 按 Java `CharTypes#trim` 删除首尾 lexer 空白。
    ///
    /// 截取单位是 UTF-16 code unit，因此不会修复或替换未配对 surrogate。
    #[must_use]
    pub fn trim(value: &JavaString) -> JavaString {
        let code_units = value.as_utf16();
        let mut start = 0;
        let mut end = code_units.len();
        while start < end && Self::is_whitespace(code_units[start]) {
            start += 1;
        }
        while start < end && Self::is_whitespace(code_units[end - 1]) {
            end -= 1;
        }
        if start == 0 && end == code_units.len() {
            value.clone()
        } else {
            JavaString::from_utf16(code_units[start..end].to_vec())
        }
    }
}
