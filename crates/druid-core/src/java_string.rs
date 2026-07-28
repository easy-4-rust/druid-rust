//! Java String 平台值对象。
//!
//! 对应 Java 平台对象：`java.lang.String`。Java String 是 UTF-16 code unit
//! 序列，允许未配对 surrogate；Rust UTF-8 String 无法无损表达该状态。

use crate::DruidError;
use std::fmt;

/// 无损 Java UTF-16 字符串。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct JavaString {
    code_units: Vec<u16>,
}

impl JavaString {
    /// 从 UTF-16 code unit 创建 Java String，不执行 surrogate 归一化。
    pub fn from_utf16(code_units: impl Into<Vec<u16>>) -> Self {
        Self {
            code_units: code_units.into(),
        }
    }

    /// 从 Rust UTF-8 字符串创建 Java String。
    pub fn from_rust_str(value: &str) -> Self {
        Self::from_utf16(value.encode_utf16().collect::<Vec<_>>())
    }

    /// 返回 UTF-16 code unit。
    pub fn as_utf16(&self) -> &[u16] {
        &self.code_units
    }

    /// 返回 Java `String#length()` 对应的 UTF-16 code unit 数量。
    pub fn len(&self) -> usize {
        self.code_units.len()
    }

    /// 返回字符串是否为空。
    pub fn is_empty(&self) -> bool {
        self.code_units.is_empty()
    }

    /// 严格转换为 Rust UTF-8 String。
    ///
    /// 存在未配对 surrogate 时返回错误，禁止静默替换。
    pub fn to_rust_string(&self) -> Result<String, DruidError> {
        String::from_utf16(&self.code_units).map_err(|error| {
            DruidError::DriverError(format!("Java String contains invalid UTF-16: {error}"))
        })
    }
}

impl From<String> for JavaString {
    fn from(value: String) -> Self {
        Self::from_rust_str(&value)
    }
}

impl From<&str> for JavaString {
    fn from(value: &str) -> Self {
        Self::from_rust_str(value)
    }
}

impl fmt::Debug for JavaString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JavaString")
            .field("utf16_length", &self.len())
            .field("valid_utf8", &String::from_utf16(&self.code_units).ok())
            .finish()
    }
}
