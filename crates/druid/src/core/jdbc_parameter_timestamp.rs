//! 对应 Java：`com.alibaba.druid.proxy.jdbc.JdbcParameterTimestamp`。

use super::{JdbcObject, JdbcParameter, JdbcParameterValue};
use chrono::NaiveDateTime;

/// SQL TIMESTAMP 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JdbcParameterTimestamp(Option<NaiveDateTime>);

impl JdbcParameterTimestamp {
    /// 创建时间戳参数。
    pub const fn new(value: Option<NaiveDateTime>) -> Self {
        Self(value)
    }
}

impl JdbcParameter for JdbcParameterTimestamp {
    fn value(&self) -> Option<JdbcParameterValue> {
        self.0
            .map(JdbcObject::Timestamp)
            .map(JdbcParameterValue::Object)
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::JdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        93
    }
}
