//! 对应 Java：`com.alibaba.druid.proxy.rdbc.RdbcParameterTimestamp`。

use super::{RdbcObject, RdbcParameter, RdbcParameterValue};
use chrono::NaiveDateTime;

/// SQL TIMESTAMP 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RdbcParameterTimestamp(Option<NaiveDateTime>);

impl RdbcParameterTimestamp {
    /// 创建时间戳参数。
    pub const fn new(value: Option<NaiveDateTime>) -> Self {
        Self(value)
    }
}

impl RdbcParameter for RdbcParameterTimestamp {
    fn value(&self) -> Option<RdbcParameterValue> {
        self.0
            .map(RdbcObject::Timestamp)
            .map(RdbcParameterValue::Object)
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::RdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        93
    }
}
