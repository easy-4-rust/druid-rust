//! 对应 Java：`com.alibaba.druid.proxy.rdbc.RdbcParameterDecimal`。

use super::{RdbcObject, RdbcParameter, RdbcParameterValue};
use bigdecimal::BigDecimal;

/// SQL DECIMAL 参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdbcParameterDecimal(Option<BigDecimal>);

impl RdbcParameterDecimal {
    /// 创建 Decimal 参数；保留 null、0、10 的值语义。
    pub fn value_of(value: Option<BigDecimal>) -> Self {
        Self(value)
    }
}

impl RdbcParameter for RdbcParameterDecimal {
    fn value(&self) -> Option<RdbcParameterValue> {
        self.0
            .clone()
            .map(RdbcObject::BigDecimal)
            .map(RdbcParameterValue::Object)
    }

    fn length(&self) -> i64 {
        0
    }

    fn calendar(&self) -> Option<super::RdbcCalendar> {
        None
    }

    fn sql_type(&self) -> i32 {
        3
    }
}
