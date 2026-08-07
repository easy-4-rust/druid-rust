//! 对应 Java：`com.alibaba.druid.proxy.rdbc.RdbcParameterLong`。

use super::{RdbcObject, RdbcParameter, RdbcParameterValue};

/// SQL BIGINT 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RdbcParameterLong(i64);

impl RdbcParameterLong {
    /// 创建长整数参数。Rust 值类型无需复制 Java 0..126 对象缓存。
    pub const fn value_of(value: i64) -> Self {
        Self(value)
    }
}

impl RdbcParameter for RdbcParameterLong {
    fn value(&self) -> Option<RdbcParameterValue> {
        Some(RdbcParameterValue::Object(RdbcObject::Long(self.0)))
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::RdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        -5
    }
}
