//! 对应 Java：`com.alibaba.druid.proxy.rdbc.RdbcParameterDate`。

use super::{RdbcObject, RdbcParameter, RdbcParameterValue};
use chrono::NaiveDate;

/// SQL DATE 参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RdbcParameterDate(Option<NaiveDate>);

impl RdbcParameterDate {
    /// 创建日期参数。
    pub const fn new(value: Option<NaiveDate>) -> Self {
        Self(value)
    }
}

impl RdbcParameter for RdbcParameterDate {
    fn value(&self) -> Option<RdbcParameterValue> {
        self.0.map(RdbcObject::Date).map(RdbcParameterValue::Object)
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::RdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        91
    }
}
