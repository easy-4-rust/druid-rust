//! `PhysicalCallableStatement` 默认强类型转换契约。
//!
//! Java oracle：
//! `DruidPooledCallableStatement` 与 `MockCallableStatement` 的标量 OUT 参数行为。

extern crate druid_core as druid;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_core::core::{
    CallableCalendar, CallableCalendarArgument, CallableInputParameter, CallableOutParameter,
    CallableParameter, DruidError, PhysicalCallableStatement, PhysicalPreparedStatement,
    RdbcObject, Value,
};
use std::any::Any;
use std::str::FromStr;
use std::sync::Mutex;

/// 可切换 OUT 值的最小物理 `CallableStatement`。
struct DefaultCallableStatement {
    output: Mutex<RdbcObject>,
}

impl DefaultCallableStatement {
    fn new(output: RdbcObject) -> Self {
        Self {
            output: Mutex::new(output),
        }
    }

    fn set_output(&self, output: RdbcObject) {
        *self
            .output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = output;
    }
}

impl PhysicalPreparedStatement for DefaultCallableStatement {
    fn sql(&self) -> &'static str {
        "{call default(?)}"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_callable(&self) -> Option<&dyn PhysicalCallableStatement> {
        Some(self)
    }

    fn close(&self) -> Result<(), DruidError> {
        Ok(())
    }

    fn is_closed(&self) -> bool {
        false
    }
}

impl PhysicalCallableStatement for DefaultCallableStatement {
    fn register_out_parameter(
        &self,
        _parameter: CallableParameter,
        _out_parameter: CallableOutParameter,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    fn set_named_parameter(
        &self,
        _parameter_name: &str,
        _parameter: CallableInputParameter,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    fn out_parameter(&self, _parameter: &CallableParameter) -> Result<RdbcObject, DruidError> {
        Ok(self
            .output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn was_null(&self) -> Result<bool, DruidError> {
        Ok(self
            .output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_null())
    }
}

#[test]
fn calendar_preserves_overload_and_time_zone_identity() {
    let shanghai = CallableCalendar::new("Asia/Shanghai").unwrap();
    assert_eq!(shanghai.time_zone_id(), "Asia/Shanghai");
    assert!(CallableCalendar::new(" ").is_err());
    assert_eq!(
        CallableCalendarArgument::default(),
        CallableCalendarArgument::Unspecified
    );
    assert_eq!(
        CallableCalendarArgument::specified(None),
        CallableCalendarArgument::Specified(None)
    );
    assert_eq!(
        CallableCalendarArgument::specified(Some(shanghai.clone())),
        CallableCalendarArgument::Specified(Some(shanghai))
    );
}

#[test]
fn default_typed_out_conversions_preserve_decimal_and_temporal_values() {
    let parameter = CallableParameter::Index(1);
    let statement = DefaultCallableStatement::new(RdbcObject::Scalar(Value::Null));
    assert_eq!(
        statement.warnings(),
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_get_warnings"
        })
    );
    assert_eq!(
        statement.clear_warnings(),
        Err(DruidError::UnsupportedOperation {
            operation: "prepared_statement_clear_warnings"
        })
    );
    assert!(statement.was_null().unwrap());
    assert_eq!(
        statement.big_decimal_out_parameter(&parameter).unwrap(),
        None
    );
    assert_eq!(
        statement
            .date_out_parameter(&parameter, &CallableCalendarArgument::Unspecified)
            .unwrap(),
        None
    );
    assert_eq!(
        statement
            .time_out_parameter(&parameter, &CallableCalendarArgument::Unspecified)
            .unwrap(),
        None
    );
    assert_eq!(
        statement
            .timestamp_out_parameter(&parameter, &CallableCalendarArgument::Unspecified)
            .unwrap(),
        None
    );

    let decimal = BigDecimal::from_str("123.4500").unwrap();
    statement.set_output(RdbcObject::BigDecimal(decimal.clone()));
    assert_eq!(
        format!("{}", RdbcObject::BigDecimal(decimal.clone())),
        "123.4500"
    );
    assert_eq!(
        statement.big_decimal_out_parameter(&parameter).unwrap(),
        Some(decimal)
    );
    let scaled = statement
        .big_decimal_out_parameter_with_scale(&parameter, 2)
        .unwrap()
        .unwrap();
    assert_eq!(scaled, BigDecimal::from_str("123.45").unwrap());
    assert_eq!(scaled.as_bigint_and_exponent().1, 2);

    let date = NaiveDate::from_ymd_opt(2026, 7, 28).unwrap();
    statement.set_output(RdbcObject::Date(date));
    assert_eq!(
        statement
            .date_out_parameter(
                &parameter,
                &CallableCalendarArgument::specified(Some(CallableCalendar::new("UTC").unwrap()))
            )
            .unwrap(),
        Some(date)
    );

    let time = NaiveTime::from_hms_nano_opt(19, 30, 45, 123_456_789).unwrap();
    statement.set_output(RdbcObject::Time(time));
    assert_eq!(
        statement
            .time_out_parameter(&parameter, &CallableCalendarArgument::Specified(None))
            .unwrap(),
        Some(time)
    );

    let timestamp = NaiveDateTime::new(date, time);
    statement.set_output(RdbcObject::Timestamp(timestamp));
    assert_eq!(
        statement
            .timestamp_out_parameter(&parameter, &CallableCalendarArgument::Unspecified)
            .unwrap(),
        Some(timestamp)
    );

    assert_eq!(format!("{}", RdbcObject::Date(date)), "2026-07-28");
    assert!(format!("{}", RdbcObject::Time(time)).contains("19:30:45"));
    assert!(format!("{}", RdbcObject::Timestamp(timestamp)).contains("2026-07-28"));
    assert_eq!(
        format!("{}", RdbcObject::Scalar(Value::String("druid".to_string()))),
        "'druid'"
    );

    statement.set_output(RdbcObject::Scalar(Value::Bool(true)));
    assert!(statement.big_decimal_out_parameter(&parameter).is_err());
    assert!(statement
        .date_out_parameter(&parameter, &CallableCalendarArgument::Unspecified)
        .is_err());
    assert!(statement
        .time_out_parameter(&parameter, &CallableCalendarArgument::Unspecified)
        .is_err());
    assert!(statement
        .timestamp_out_parameter(&parameter, &CallableCalendarArgument::Unspecified)
        .is_err());
}
