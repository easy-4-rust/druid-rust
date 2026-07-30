//! 对应 Java：`com.alibaba.druid.proxy.jdbc.JdbcParameterImpl`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/proxy/jdbc/JdbcParameterImpl.java`。

use super::{JdbcCalendar, JdbcParameter, JdbcParameterValue};

/// 携带任意 SQL 类型、长度、Calendar 与 scaleOrLength 的通用参数。
#[derive(Debug, Clone, PartialEq)]
pub struct JdbcParameterImpl {
    sql_type: i32,
    value: Option<JdbcParameterValue>,
    length: i64,
    calendar: Option<JdbcCalendar>,
    scale_or_length: i32,
}

impl JdbcParameterImpl {
    /// 构造完整参数。
    pub fn new(
        sql_type: i32,
        value: Option<JdbcParameterValue>,
        length: i64,
        calendar: Option<JdbcCalendar>,
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
    pub fn with_value(sql_type: i32, value: Option<JdbcParameterValue>) -> Self {
        Self::new(sql_type, value, -1, None, -1)
    }

    /// 构造声明长度的参数。
    pub fn with_length(sql_type: i32, value: Option<JdbcParameterValue>, length: i64) -> Self {
        Self::new(sql_type, value, length, None, -1)
    }

    /// 构造声明 Calendar 的参数。
    pub fn with_calendar(
        sql_type: i32,
        value: Option<JdbcParameterValue>,
        calendar: Option<JdbcCalendar>,
    ) -> Self {
        Self::new(sql_type, value, -1, calendar, -1)
    }

    /// 返回 `setObject` 的 scaleOrLength；未声明时为 -1。
    pub const fn scale_or_length(&self) -> i32 {
        self.scale_or_length
    }
}

impl JdbcParameter for JdbcParameterImpl {
    fn value(&self) -> Option<JdbcParameterValue> {
        self.value.clone()
    }

    fn length(&self) -> i64 {
        self.length
    }

    fn calendar(&self) -> Option<JdbcCalendar> {
        self.calendar.clone()
    }

    fn sql_type(&self) -> i32 {
        self.sql_type
    }
}
