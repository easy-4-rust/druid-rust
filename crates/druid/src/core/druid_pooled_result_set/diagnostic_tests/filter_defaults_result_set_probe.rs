use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcObject, RdbcTargetType, RdbcTypeMap, Value};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
/// 完整原默认委托探针，保留原生重载日志与返回值。对应 Java: java.sql.ResultSet。
pub(super) struct FilterDefaultsResultSetProbe {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    closed: bool,
}
impl FilterDefaultsResultSetProbe {
    /// 创建测试迁移桥或原生探针；参数为构造输入，返回新的测试对象。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>, _close_error: bool) -> Self {
        Self {
            calls,
            closed: false,
        }
    }
    fn record(&self, event: String) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }
    fn calendar(arg: &RdbcCalendarArgument) -> String {
        match arg {
            RdbcCalendarArgument::Unspecified => "unspecified".into(),
            RdbcCalendarArgument::Specified(None) => "specified:null".into(),
            RdbcCalendarArgument::Specified(Some(c)) => format!("specified:{}", c.time_zone_id()),
        }
    }
    fn map(map: Option<&RdbcTypeMap>) -> String {
        match map {
            None => "null".into(),
            Some(m) if m.is_empty() => "empty".into(),
            Some(m) => {
                let mut entries = m
                    .mappings()
                    .iter()
                    .map(|(n, t)| format!("{n}={t:?}"))
                    .collect::<Vec<_>>();
                entries.sort();
                entries.join(",")
            }
        }
    }
    fn read(
        &self,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        let by_label = label.is_some();
        let target = label
            .map(str::to_owned)
            .unwrap_or_else(|| column.unwrap().get().to_string());
        let (name, extra, value) = match request {
            ResultSetRead::String => (
                "string",
                String::new(),
                if by_label {
                    Value::String("label-string".into())
                } else {
                    Value::String("index-string".into())
                },
            ),
            ResultSetRead::Boolean => (
                "boolean",
                String::new(),
                if by_label {
                    Value::Bool(false)
                } else {
                    Value::Bool(true)
                },
            ),
            ResultSetRead::Byte => (
                "byte",
                String::new(),
                if by_label {
                    Value::Int(9)
                } else {
                    Value::Int(8)
                },
            ),
            ResultSetRead::Short => (
                "short",
                String::new(),
                if by_label {
                    Value::Int(17)
                } else {
                    Value::Int(16)
                },
            ),
            ResultSetRead::Int => (
                "int",
                String::new(),
                if by_label {
                    Value::Int(33)
                } else {
                    Value::Int(32)
                },
            ),
            ResultSetRead::Long => (
                "long",
                String::new(),
                if by_label {
                    Value::Int(65)
                } else {
                    Value::Int(64)
                },
            ),
            ResultSetRead::Float => (
                "float",
                String::new(),
                if by_label {
                    Value::Float(2.25)
                } else {
                    Value::Float(1.25)
                },
            ),
            ResultSetRead::Double => (
                "double",
                String::new(),
                if by_label {
                    Value::Float(4.5)
                } else {
                    Value::Float(3.5)
                },
            ),
            ResultSetRead::Bytes => (
                "bytes",
                String::new(),
                if by_label {
                    Value::Bytes(vec![3, 4])
                } else {
                    Value::Bytes(vec![1, 2])
                },
            ),
            ResultSetRead::BigDecimal(scale) => (
                "big_decimal",
                format!(":{scale:?}"),
                Value::Decimal(
                    BigDecimal::from_str(if by_label { "56.780" } else { "12.340" }).unwrap(),
                ),
            ),
            ResultSetRead::Date(arg) => (
                "date",
                format!(":{}", Self::calendar(arg)),
                Value::Date(if by_label {
                    NaiveDate::from_ymd_opt(2025, 2, 3).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(2025, 1, 2).unwrap()
                }),
            ),
            ResultSetRead::Time(arg) => (
                "time",
                format!(":{}", Self::calendar(arg)),
                Value::Time(if by_label {
                    NaiveTime::from_hms_opt(6, 7, 8).unwrap()
                } else {
                    NaiveTime::from_hms_opt(3, 4, 5).unwrap()
                }),
            ),
            ResultSetRead::Timestamp(arg) => (
                "timestamp",
                format!(":{}", Self::calendar(arg)),
                Value::Timestamp(if by_label {
                    NaiveDate::from_ymd_opt(2025, 2, 3)
                        .unwrap()
                        .and_hms_opt(6, 7, 8)
                        .unwrap()
                } else {
                    NaiveDate::from_ymd_opt(2025, 1, 2)
                        .unwrap()
                        .and_hms_opt(3, 4, 5)
                        .unwrap()
                }),
            ),
            _ => {
                return Err(RdbcError::DriverError(
                    "unexpected original default read".into(),
                ))
            }
        };
        self.record(format!(
            "physical:{name}{}:{target}{extra}",
            if by_label { "_by_label" } else { "" }
        ));
        Ok(RdbcObject::Scalar(value))
    }
}
druid_rdbc::impl_standard_wrapper!(FilterDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for FilterDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.record("physical:next".into());
        Ok(true)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.record("physical:close".into());
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.record(format!("physical:find_column:{label}"));
        ColumnIndex::new(23)
    }
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        self.read(Some(column), None, request)
    }
    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        self.read(None, Some(label), request)
    }
    fn object(&self, column: ColumnIndex) -> Result<RdbcObject, RdbcError> {
        self.record(format!("physical:value:{}", column.get()));
        Ok(RdbcObject::Scalar(Value::Int(column.get() as i64)))
    }
    fn object_by_label(&self, label: &str) -> Result<RdbcObject, RdbcError> {
        self.record(format!("physical:value_by_label:{label}"));
        Ok(RdbcObject::Scalar(Value::String(label.into())))
    }
    fn object_with_type_map(
        &self,
        column: ColumnIndex,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        let map = Self::map(map);
        self.record(format!(
            "physical:object_with_type_map:{}:{map}",
            column.get()
        ));
        Ok(RdbcObject::String(format!(
            "index-map:{}:{map}",
            column.get()
        )))
    }
    fn object_with_type_map_by_label(
        &self,
        label: &str,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        let map = Self::map(map);
        self.record(format!(
            "physical:object_by_label_with_type_map:{label}:{map}"
        ));
        Ok(RdbcObject::String(format!("label-map:{label}:{map}")))
    }
    fn object_as(
        &self,
        column: ColumnIndex,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.record(format!("physical:object_as:{}:{target:?}", column.get()));
        Ok(RdbcObject::String(format!(
            "index-typed:{}:{target:?}",
            column.get()
        )))
    }
    fn object_as_by_label(
        &self,
        label: &str,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.record(format!("physical:object_by_label_as:{label}:{target:?}"));
        Ok(RdbcObject::String(format!(
            "label-typed:{label}:{target:?}"
        )))
    }
}
