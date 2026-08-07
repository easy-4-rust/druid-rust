//! 对应 Java：`com.alibaba.druid.proxy.rdbc.RdbcParameterString`。

use super::{RdbcObject, RdbcParameter, RdbcParameterValue};

/// SQL VARCHAR 参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdbcParameterString(Option<String>);

impl RdbcParameterString {
    /// 创建字符串参数；`None` 对应直接构造 Java 对象时的 null。
    pub fn new(value: Option<String>) -> Self {
        Self(value)
    }

    /// 创建 Java 共享 empty 值的等价空字符串参数。
    pub fn empty() -> Self {
        Self(Some(String::new()))
    }
}

impl RdbcParameter for RdbcParameterString {
    fn value(&self) -> Option<RdbcParameterValue> {
        self.0
            .clone()
            .map(RdbcObject::String)
            .map(RdbcParameterValue::Object)
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::RdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        12
    }
}
