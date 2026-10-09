//! 完整强类型读取原案例的短路 Filter，不访问真实驱动游标。
use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveTime};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead};
use druid_rdbc::types::RdbcCalendarArgument;
use std::sync::{Arc, Mutex};
/// 保存原重载调用记录。对应 Java: com.alibaba.druid.filter.Filter。
pub(super) struct StrongGetterShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}
impl ResultSetFilter for StrongGetterShortCircuitFilter {
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        let (stem, calendar, scale) = match request {
            ResultSetRead::BigDecimal(scale) => ("big_decimal", None, *scale),
            ResultSetRead::Date(calendar) => ("date", Some(calendar), None),
            ResultSetRead::Time(calendar) => ("time", Some(calendar), None),
            ResultSetRead::Timestamp(calendar) => ("timestamp", Some(calendar), None),
            _ => return next(),
        };
        let suffix = if label.is_some() { "_by_label" } else { "" };
        let target = label
            .map(str::to_owned)
            .unwrap_or_else(|| column.unwrap().get().to_string());
        let calendar_suffix = match calendar {
            Some(RdbcCalendarArgument::Specified(None)) => Some("specified:null".to_owned()),
            Some(RdbcCalendarArgument::Specified(Some(value))) => {
                Some(format!("specified:{}", value.time_zone_id()))
            }
            _ => None,
        };
        let call = if let Some(scale) = scale {
            format!("result_set_get_{stem}{suffix}_with_scale:{target}:{scale}")
        } else if let Some(calendar) = &calendar_suffix {
            format!("result_set_get_{stem}{suffix}_with_calendar:{target}:{calendar}")
        } else {
            format!("result_set_get_{stem}{suffix}:{target}")
        };
        self.calls.lock().unwrap().push(call);
        if label == Some("fail") {
            return Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "filtered temporal failure".to_owned(),
            )));
        }
        let ordinal = if calendar_suffix.is_some() {
            3
        } else if label.is_some() {
            2
        } else {
            1
        };
        Ok(match stem {
            "big_decimal" => {
                Value::Decimal(BigDecimal::from(match (label.is_some(), scale.is_some()) {
                    (false, false) => 101,
                    (true, false) => 102,
                    (false, true) => 103,
                    (true, true) => 104,
                }))
            }
            "date" => Value::Date(NaiveDate::from_ymd_opt(2031, 1, ordinal).unwrap()),
            "time" => {
                Value::Time(NaiveTime::from_hms_opt(ordinal, ordinal + 1, ordinal + 2).unwrap())
            }
            "timestamp" => Value::Timestamp(
                NaiveDate::from_ymd_opt(2031, 1, ordinal)
                    .unwrap()
                    .and_hms_opt(ordinal, ordinal + 1, ordinal + 2)
                    .unwrap(),
            ),
            _ => unreachable!(),
        })
    }
}
