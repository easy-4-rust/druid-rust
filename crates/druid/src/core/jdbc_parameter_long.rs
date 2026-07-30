//! 对应 Java：`com.alibaba.druid.proxy.jdbc.JdbcParameterLong`。

use super::{JdbcObject, JdbcParameter, JdbcParameterValue};

/// SQL BIGINT 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JdbcParameterLong(i64);

impl JdbcParameterLong {
    /// 创建长整数参数。Rust 值类型无需复制 Java 0..126 对象缓存。
    pub const fn value_of(value: i64) -> Self {
        Self(value)
    }
}

impl JdbcParameter for JdbcParameterLong {
    fn value(&self) -> Option<JdbcParameterValue> {
        Some(JdbcParameterValue::Object(JdbcObject::Long(self.0)))
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::JdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        -5
    }
}
