//! `FilterDefaultsChainHarness` 的 decimal/时间/对象/警告读取族。
//!
//! 拆分自 `filter_defaults_chain_harness.rs`，按读取值族归类，父文件保留
//! 构造、链事件与标量数值读取。

use super::*;

impl FilterDefaultsChainHarness {
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(crate) fn result_set_get_big_decimal(
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
    pub(crate) fn result_set_get_big_decimal_with_scale(
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
    pub(crate) fn result_set_get_big_decimal_by_label(
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
    pub(crate) fn result_set_get_big_decimal_by_label_with_scale(
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
    pub(crate) fn result_set_get_date(
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
    pub(crate) fn result_set_get_date_with_calendar(
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
    pub(crate) fn result_set_get_date_by_label(
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
    pub(crate) fn result_set_get_date_by_label_with_calendar(
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
    pub(crate) fn result_set_get_time(
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
    pub(crate) fn result_set_get_time_with_calendar(
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
    pub(crate) fn result_set_get_time_by_label(
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
    pub(crate) fn result_set_get_time_by_label_with_calendar(
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
    pub(crate) fn result_set_get_timestamp(
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
    pub(crate) fn result_set_get_timestamp_with_calendar(
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
    pub(crate) fn result_set_get_timestamp_by_label(
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
    pub(crate) fn result_set_get_timestamp_by_label_with_calendar(
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
    pub(crate) fn result_set_get_object(
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
    pub(crate) fn result_set_get_object_with_type_map(
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
    pub(crate) fn result_set_get_object_typed(
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
    pub(crate) fn result_set_get_object_by_label(
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
    pub(crate) fn result_set_get_object_by_label_with_type_map(
        &self,
        r: &mut dyn ResultSet,
        c: &ResultSetFilterContext,
        label: &str,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        self.object(r, c, None, Some(label), ResultSetObjectRead::TypeMap(map))
    }
    /// 委托原读取重载至实际标准链；参数为游标、上下文、原列身份和重载参数，返回原值或错误。
    pub(crate) fn result_set_get_object_typed_by_label(
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
    pub(crate) fn result_set_warnings(
        &self,
        r: &mut dyn ResultSet,
        _c: &ResultSetFilterContext,
    ) -> Result<Option<SqlWarning>, DruidError> {
        r.warnings().map_err(DruidError::from)
    }
    /// 委托标准警告清理；参数为游标和上下文，返回成功或原能力错误。
    pub(crate) fn result_set_clear_warnings(
        &self,
        r: &mut dyn ResultSet,
        _c: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        r.clear_warnings().map_err(DruidError::from)
    }
}
