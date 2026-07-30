//! 对应 Java：`com.alibaba.druid.proxy.jdbc.JdbcParameterString`。

use super::{JdbcObject, JdbcParameter, JdbcParameterValue};

/// SQL VARCHAR 参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JdbcParameterString(Option<String>);

impl JdbcParameterString {
    /// 创建字符串参数；`None` 对应直接构造 Java 对象时的 null。
    pub fn new(value: Option<String>) -> Self {
        Self(value)
    }

    /// 创建 Java 共享 empty 值的等价空字符串参数。
    pub fn empty() -> Self {
        Self(Some(String::new()))
    }
}

impl JdbcParameter for JdbcParameterString {
    fn value(&self) -> Option<JdbcParameterValue> {
        self.0
            .clone()
            .map(JdbcObject::String)
            .map(JdbcParameterValue::Object)
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::JdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        12
    }
}
