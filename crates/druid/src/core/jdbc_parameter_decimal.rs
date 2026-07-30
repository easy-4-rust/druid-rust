//! 对应 Java：`com.alibaba.druid.proxy.jdbc.JdbcParameterDecimal`。

use super::{JdbcObject, JdbcParameter, JdbcParameterValue};
use bigdecimal::BigDecimal;

/// SQL DECIMAL 参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JdbcParameterDecimal(Option<BigDecimal>);

impl JdbcParameterDecimal {
    /// 创建 Decimal 参数；保留 null、0、10 的值语义。
    pub fn value_of(value: Option<BigDecimal>) -> Self {
        Self(value)
    }
}

impl JdbcParameter for JdbcParameterDecimal {
    fn value(&self) -> Option<JdbcParameterValue> {
        self.0
            .clone()
            .map(JdbcObject::BigDecimal)
            .map(JdbcParameterValue::Object)
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::JdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        3
    }
}
