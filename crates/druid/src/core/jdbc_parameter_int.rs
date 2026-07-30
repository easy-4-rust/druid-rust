//! 对应 Java：`com.alibaba.druid.proxy.jdbc.JdbcParameterInt`。

use super::{JdbcObject, JdbcParameter, JdbcParameterValue};

/// SQL INTEGER 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JdbcParameterInt(i32);

impl JdbcParameterInt {
    /// 创建整数参数。Rust 值类型无需复制 Java 0..126 对象缓存。
    pub const fn value_of(value: i32) -> Self {
        Self(value)
    }
}

impl JdbcParameter for JdbcParameterInt {
    fn value(&self) -> Option<JdbcParameterValue> {
        Some(JdbcParameterValue::Object(JdbcObject::Integer(self.0)))
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::JdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        4
    }
}
