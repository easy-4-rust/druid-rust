use crate::core::{
    DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext, ResultSetObjectRead,
    StandardResultSetOpenContext,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::metadata::ResultSetMetaDataSnapshot;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcObject, RdbcTargetType, RdbcTypeMap, Value};
use std::sync::Arc;
/// 测试迁移桥：只把原重载转成标准描述符，所有读取运行实际FilterChain。对应 Java: FilterChainImpl。
pub(super) struct FilterDefaultsChainHarness {
    inner: FilterChain,
}
impl FilterDefaultsChainHarness {
    /// 创建测试迁移桥或原生探针；参数为构造输入，返回新的测试对象。
    pub(super) fn new() -> Self {
        Self {
            inner: FilterChain::new(),
        }
    }
    /// 登记默认过滤器；参数为拥有型Filter，返回单位值。
    pub(super) fn add_result_set(&mut self, f: Arc<dyn ResultSetFilter>) {
        self.inner.add_result_set(f);
    }
    /// 分派原开放事件；参数为上下文，返回成功或原错误。
    pub(super) fn result_set_open_after(
        &self,
        c: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.inner.standard_result_set_open_after_with_proxy(
            &mut StandardResultSetOpenContext::new(
                c,
                Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())),
            ),
        )
    }
    /// 推进原生游标；参数为游标和上下文，返回行状态或原错误。
    pub(super) async fn result_set_next(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
    ) -> Result<bool, DruidError> {
        self.inner.standard_result_set_next(r, c).await
    }
    /// 关闭原生游标；参数为游标和上下文，返回成功或原错误。
    pub(super) async fn result_set_close(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.inner.standard_result_set_close(r, c).await
    }
    fn read(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: ResultSetRead,
    ) -> Result<Value, DruidError> {
        self.inner
            .standard_result_set_typed_value(r, c, column, &request, label)
    }
    fn object(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: ResultSetObjectRead<'_>,
    ) -> Result<RdbcObject, DruidError> {
        self.inner
            .standard_result_set_object(r, c, column, &request, label)
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_string(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<Option<String>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::String,
        )? {
            Value::String(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_string_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<String>, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::String)? {
            Value::String(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_boolean(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<bool, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Boolean,
        )? {
            Value::Bool(v) => Ok(v),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_boolean_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<bool, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::Boolean)? {
            Value::Bool(v) => Ok(v),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_byte(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<i8, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Byte,
        )? {
            Value::Int(v) => Ok(v.try_into().unwrap()),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_byte_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<i8, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::Byte)? {
            Value::Int(v) => Ok(v.try_into().unwrap()),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_short(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<i16, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Short,
        )? {
            Value::Int(v) => Ok(v.try_into().unwrap()),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_short_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<i16, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::Short)? {
            Value::Int(v) => Ok(v.try_into().unwrap()),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_int(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<i32, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Int,
        )? {
            Value::Int(v) => Ok(v.try_into().unwrap()),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_int_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<i32, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::Int)? {
            Value::Int(v) => Ok(v.try_into().unwrap()),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_long(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<i64, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Long,
        )? {
            Value::Int(v) => Ok(v),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_long_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<i64, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::Long)? {
            Value::Int(v) => Ok(v),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_float(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<f32, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Float,
        )? {
            Value::Float(v) => Ok(v as f32),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_float_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<f32, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::Float)? {
            Value::Float(v) => Ok(v as f32),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_double(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<f64, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Double,
        )? {
            Value::Float(v) => Ok(v),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_double_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<f64, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::Double)? {
            Value::Float(v) => Ok(v),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_bytes(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<Option<Vec<u8>>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Bytes,
        )? {
            Value::Bytes(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_bytes_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<Vec<u8>>, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::Bytes)? {
            Value::Bytes(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_big_decimal(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::BigDecimal(None),
        )? {
            Value::Decimal(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_big_decimal_with_scale(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::BigDecimal(Some(scale)),
        )? {
            Value::Decimal(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_big_decimal_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        match self.read(r, c, None, Some(label), ResultSetRead::BigDecimal(None))? {
            Value::Decimal(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_big_decimal_by_label_with_scale(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        match self.read(
            r,
            c,
            None,
            Some(label),
            ResultSetRead::BigDecimal(Some(scale)),
        )? {
            Value::Decimal(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_date(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<Option<NaiveDate>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Date(RdbcCalendarArgument::Unspecified),
        )? {
            Value::Date(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_date_with_calendar(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Date(calendar.clone()),
        )? {
            Value::Date(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_date_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<NaiveDate>, DruidError> {
        match self.read(
            r,
            c,
            None,
            Some(label),
            ResultSetRead::Date(RdbcCalendarArgument::Unspecified),
        )? {
            Value::Date(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_date_by_label_with_calendar(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        match self.read(
            r,
            c,
            None,
            Some(label),
            ResultSetRead::Date(calendar.clone()),
        )? {
            Value::Date(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_time(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<Option<NaiveTime>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Time(RdbcCalendarArgument::Unspecified),
        )? {
            Value::Time(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_time_with_calendar(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Time(calendar.clone()),
        )? {
            Value::Time(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_time_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<NaiveTime>, DruidError> {
        match self.read(
            r,
            c,
            None,
            Some(label),
            ResultSetRead::Time(RdbcCalendarArgument::Unspecified),
        )? {
            Value::Time(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_time_by_label_with_calendar(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        match self.read(
            r,
            c,
            None,
            Some(label),
            ResultSetRead::Time(calendar.clone()),
        )? {
            Value::Time(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_timestamp(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Timestamp(RdbcCalendarArgument::Unspecified),
        )? {
            Value::Timestamp(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_timestamp_with_calendar(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        match self.read(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetRead::Timestamp(calendar.clone()),
        )? {
            Value::Timestamp(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_timestamp_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        match self.read(
            r,
            c,
            None,
            Some(label),
            ResultSetRead::Timestamp(RdbcCalendarArgument::Unspecified),
        )? {
            Value::Timestamp(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_timestamp_by_label_with_calendar(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
        calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        match self.read(
            r,
            c,
            None,
            Some(label),
            ResultSetRead::Timestamp(calendar.clone()),
        )? {
            Value::Timestamp(v) => Ok(Some(v)),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected scalar {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_object(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
    ) -> Result<Value, DruidError> {
        match self.object(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetObjectRead::Ordinary,
        )? {
            RdbcObject::Scalar(v) => Ok(v),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected object {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_object_with_type_map(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        self.object(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetObjectRead::TypeMap(map),
        )
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_object_typed(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        column: usize,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        self.object(
            r,
            c,
            Some(ColumnIndex::new(column).unwrap()),
            None,
            ResultSetObjectRead::Typed(target),
        )
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_object_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
    ) -> Result<Value, DruidError> {
        match self.object(r, c, None, Some(label), ResultSetObjectRead::Ordinary)? {
            RdbcObject::Scalar(v) => Ok(v),
            other => Err(DruidError::from(RdbcError::DriverError(format!(
                "unexpected object {other:?}"
            )))),
        }
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_object_by_label_with_type_map(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        self.object(r, c, None, Some(label), ResultSetObjectRead::TypeMap(map))
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(super) fn result_set_get_object_typed_by_label(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        self.object(r, c, None, Some(label), ResultSetObjectRead::Typed(target))
    }
    // 标准warnings没有独立around hook，保持默认原生能力错误，不伪造支持。
    /// 委托标准警告读取；参数为游标和上下文，返回警告或原能力错误。
    pub(super) fn result_set_warnings(
        &self,
        r: &mut dyn ResultSet,
        _c: &ResultSetFilterContext,
    ) -> Result<Option<SqlWarning>, DruidError> {
        r.warnings().map_err(DruidError::from)
    }
    /// 委托标准警告清理；参数为游标和上下文，返回成功或原能力错误。
    pub(super) fn result_set_clear_warnings(
        &self,
        r: &mut dyn ResultSet,
        _c: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        r.clear_warnings().map_err(DruidError::from)
    }
}
