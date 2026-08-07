//! 对应 Java：`com.alibaba.druid.proxy.rdbc.RdbcParameterImpl`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/proxy/rdbc/RdbcParameterImpl.java`。

use super::{RdbcCalendar, RdbcParameter, RdbcParameterValue};

/// 携带任意 SQL 类型、长度、Calendar 与 scaleOrLength 的通用参数。
#[derive(Debug, Clone, PartialEq)]
pub struct RdbcParameterImpl {
    sql_type: i32,
    value: Option<RdbcParameterValue>,
    length: i64,
    calendar: Option<RdbcCalendar>,
    scale_or_length: i32,
}

impl RdbcParameterImpl {
    /// 构造完整参数。
    pub fn new(
        sql_type: i32,
        value: Option<RdbcParameterValue>,
        length: i64,
        calendar: Option<RdbcCalendar>,
        scale_or_length: i32,
    ) -> Self {
        Self {
            sql_type,
            value,
            length,
            calendar,
            scale_or_length,
        }
    }

    /// 构造未声明长度、Calendar 和 scale 的参数。
    pub fn with_value(sql_type: i32, value: Option<RdbcParameterValue>) -> Self {
        Self::new(sql_type, value, -1, None, -1)
    }

    /// 构造声明长度的参数。
    pub fn with_length(sql_type: i32, value: Option<RdbcParameterValue>, length: i64) -> Self {
        Self::new(sql_type, value, length, None, -1)
    }

    /// 构造声明 Calendar 的参数。
    pub fn with_calendar(
        sql_type: i32,
        value: Option<RdbcParameterValue>,
        calendar: Option<RdbcCalendar>,
    ) -> Self {
        Self::new(sql_type, value, -1, calendar, -1)
    }

    /// 返回 `setObject` 的 scaleOrLength；未声明时为 -1。
    pub const fn scale_or_length(&self) -> i32 {
        self.scale_or_length
    }
}

impl RdbcParameter for RdbcParameterImpl {
    fn value(&self) -> Option<RdbcParameterValue> {
        self.value.clone()
    }

    fn length(&self) -> i64 {
        self.length
    }

    fn calendar(&self) -> Option<RdbcCalendar> {
        self.calendar.clone()
    }

    fn sql_type(&self) -> i32 {
        self.sql_type
    }
}
