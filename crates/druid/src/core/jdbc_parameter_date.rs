//! 对应 Java：`com.alibaba.druid.proxy.jdbc.JdbcParameterDate`。

use super::{JdbcObject, JdbcParameter, JdbcParameterValue};
use chrono::NaiveDate;

/// SQL DATE 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JdbcParameterDate(Option<NaiveDate>);

impl JdbcParameterDate {
    /// 创建日期参数。
    pub const fn new(value: Option<NaiveDate>) -> Self {
        Self(value)
    }
}

impl JdbcParameter for JdbcParameterDate {
    fn value(&self) -> Option<JdbcParameterValue> {
        self.0.map(JdbcObject::Date).map(JdbcParameterValue::Object)
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::JdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        91
    }
}
