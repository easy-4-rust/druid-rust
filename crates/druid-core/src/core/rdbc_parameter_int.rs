//! 对应 Java：`com.alibaba.druid.proxy.rdbc.RdbcParameterInt`。

use super::{RdbcObject, RdbcParameter, RdbcParameterValue};

/// SQL INTEGER 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RdbcParameterInt(i32);

impl RdbcParameterInt {
    /// 创建整数参数。Rust 值类型无需复制 Java 0..126 对象缓存。
    pub const fn value_of(value: i32) -> Self {
        Self(value)
    }
}

impl RdbcParameter for RdbcParameterInt {
    fn value(&self) -> Option<RdbcParameterValue> {
        Some(RdbcParameterValue::Object(RdbcObject::Integer(self.0)))
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::RdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        4
    }
}
