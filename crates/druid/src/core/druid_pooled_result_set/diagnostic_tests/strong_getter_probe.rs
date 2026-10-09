use super::strong_getter_call::StrongGetterCall;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead, ResultSetUpdate};
use druid_rdbc::types::{
    RdbcBigDecimal, RdbcNaiveDate, RdbcNaiveTime, RdbcObject, RdbcTargetType, RdbcTypeMap, Value,
};
use std::sync::{Arc, Mutex};
/// 保留 BASE 真正驱动强类型方法的观察。对应 Java: java.sql.ResultSet。
#[derive(Debug)]
pub(super) struct StrongGetterProbe {
    calls: Arc<Mutex<Vec<StrongGetterCall>>>,
    closed: bool,
}
impl StrongGetterProbe {
    /// 创建空调用记录的原始游标；参数为共享记录，返回驱动探针。
    pub(super) fn new(calls: Arc<Mutex<Vec<StrongGetterCall>>>) -> Self {
        Self {
            calls,
            closed: false,
        }
    }
}
druid_rdbc::impl_standard_wrapper!(StrongGetterProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for StrongGetterProbe {
    fn object_with_type_map(
        &self,
        column: ColumnIndex,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ObjectMapIndex(column.get(), map.cloned()));
        Ok(RdbcObject::Scalar(Value::Int(71)))
    }
    fn object_with_type_map_by_label(
        &self,
        label: &str,
        map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ObjectMapLabel(
                label.to_owned(),
                map.cloned(),
            ));
        Ok(RdbcObject::Scalar(Value::Int(72)))
    }
    fn object_as(
        &self,
        column: ColumnIndex,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ObjectTypedIndex(
                column.get(),
                target.clone(),
            ));
        Ok(RdbcObject::String("typed-index".to_owned()))
    }
    fn object_as_by_label(
        &self,
        label: &str,
        target: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.calls
            .lock()
            .unwrap()
            .push(StrongGetterCall::ObjectTypedLabel(
                label.to_owned(),
                target.clone(),
            ));
        Ok(RdbcObject::String("typed-label".to_owned()))
    }
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        if label == "mapped" {
            ColumnIndex::new(2)
        } else {
            Err(RdbcError::InvalidArgument(
                "native label must reach the getter".to_owned(),
            ))
        }
    }
    fn read_object(
        &self,
        column: ColumnIndex,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        let (call, value) = match request {
            ResultSetRead::BigDecimal(arg) => (
                StrongGetterCall::BigDecimalIndex(column.get(), *arg),
                Value::Decimal(RdbcBigDecimal::from(1)),
            ),
            ResultSetRead::Date(arg) => (
                StrongGetterCall::DateIndex(column.get(), arg.clone()),
                Value::Date(RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap()),
            ),
            ResultSetRead::Time(arg) => (
                StrongGetterCall::TimeIndex(column.get(), arg.clone()),
                Value::Time(RdbcNaiveTime::from_hms_opt(13, 14, 15).unwrap()),
            ),
            ResultSetRead::Timestamp(arg) => (
                StrongGetterCall::TimestampIndex(column.get(), arg.clone()),
                Value::Timestamp(
                    RdbcNaiveDate::from_ymd_opt(2026, 7, 29)
                        .unwrap()
                        .and_hms_opt(13, 14, 15)
                        .unwrap(),
                ),
            ),
            ResultSetRead::Ref => (StrongGetterCall::RefIndex(column.get()), Value::Null),
            ResultSetRead::Blob => (StrongGetterCall::BlobIndex(column.get()), Value::Null),
            ResultSetRead::Clob => (StrongGetterCall::ClobIndex(column.get()), Value::Null),
            ResultSetRead::Array => (StrongGetterCall::ArrayIndex(column.get()), Value::Null),
            ResultSetRead::Url => (StrongGetterCall::UrlIndex(column.get()), Value::Null),
            ResultSetRead::RowId => (StrongGetterCall::RowIdIndex(column.get()), Value::Null),
            ResultSetRead::NClob => (StrongGetterCall::NClobIndex(column.get()), Value::Null),
            ResultSetRead::SqlXml => (StrongGetterCall::SqlXmlIndex(column.get()), Value::Null),
            _ => {
                return Err(RdbcError::DriverError(
                    "unexpected temporal/resource descriptor".to_owned(),
                ))
            }
        };
        self.calls.lock().unwrap().push(call);
        Ok(RdbcObject::Scalar(value))
    }
    fn read_object_by_label(
        &self,
        label: &str,
        request: &ResultSetRead,
    ) -> Result<RdbcObject, RdbcError> {
        let (call, value) = match request {
            ResultSetRead::BigDecimal(arg) => (
                StrongGetterCall::BigDecimalLabel(label.to_owned(), *arg),
                Value::Decimal(RdbcBigDecimal::from(1)),
            ),
            ResultSetRead::Date(arg) => (
                StrongGetterCall::DateLabel(label.to_owned(), arg.clone()),
                Value::Date(RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap()),
            ),
            ResultSetRead::Time(arg) => (
                StrongGetterCall::TimeLabel(label.to_owned(), arg.clone()),
                Value::Time(RdbcNaiveTime::from_hms_opt(13, 14, 15).unwrap()),
            ),
            ResultSetRead::Timestamp(arg) => (
                StrongGetterCall::TimestampLabel(label.to_owned(), arg.clone()),
                Value::Timestamp(
                    RdbcNaiveDate::from_ymd_opt(2026, 7, 29)
                        .unwrap()
                        .and_hms_opt(13, 14, 15)
                        .unwrap(),
                ),
            ),
            ResultSetRead::Ref => (StrongGetterCall::RefLabel(label.to_owned()), Value::Null),
            ResultSetRead::Blob => (StrongGetterCall::BlobLabel(label.to_owned()), Value::Null),
            ResultSetRead::Clob => (StrongGetterCall::ClobLabel(label.to_owned()), Value::Null),
            ResultSetRead::Array => (StrongGetterCall::ArrayLabel(label.to_owned()), Value::Null),
            ResultSetRead::Url => (StrongGetterCall::UrlLabel(label.to_owned()), Value::Null),
            ResultSetRead::RowId => (StrongGetterCall::RowIdLabel(label.to_owned()), Value::Null),
            ResultSetRead::NClob => (StrongGetterCall::NClobLabel(label.to_owned()), Value::Null),
            ResultSetRead::SqlXml => (StrongGetterCall::SqlXmlLabel(label.to_owned()), Value::Null),
            _ => {
                return Err(RdbcError::DriverError(
                    "unexpected temporal/resource descriptor".to_owned(),
                ))
            }
        };
        self.calls.lock().unwrap().push(call);
        Ok(RdbcObject::Scalar(value))
    }
    fn update_value(
        &mut self,
        column: ColumnIndex,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        let call = match update {
            ResultSetUpdate::Ref(value) => {
                StrongGetterCall::UpdateRefIndex(column.get(), value.is_some())
            }
            ResultSetUpdate::Blob(value) => {
                StrongGetterCall::UpdateBlobIndex(column.get(), value.is_some())
            }
            ResultSetUpdate::Clob(value) => {
                StrongGetterCall::UpdateClobIndex(column.get(), value.is_some())
            }
            ResultSetUpdate::Array(value) => {
                StrongGetterCall::UpdateArrayIndex(column.get(), value.is_some())
            }
            ResultSetUpdate::RowId(value) => {
                StrongGetterCall::UpdateRowIdIndex(column.get(), value.is_some())
            }
            ResultSetUpdate::NClob(value) => {
                StrongGetterCall::UpdateNClobIndex(column.get(), value.is_some())
            }
            ResultSetUpdate::SqlXml(value) => {
                StrongGetterCall::UpdateSqlXmlIndex(column.get(), value.is_some())
            }
            ResultSetUpdate::BlobStream { stream, length } => {
                StrongGetterCall::UpdateBlobStreamIndex(column.get(), stream.is_some(), length)
            }
            ResultSetUpdate::ClobReader { reader, length } => {
                StrongGetterCall::UpdateClobReaderIndex(column.get(), reader.is_some(), length)
            }
            ResultSetUpdate::NClobReader { reader, length } => {
                StrongGetterCall::UpdateNClobReaderIndex(column.get(), reader.is_some(), length)
            }
            _ => {
                return Err(RdbcError::InvalidArgument(
                    "resource update identity lost".to_owned(),
                ))
            }
        };
        self.calls.lock().unwrap().push(call);
        Ok(())
    }
    fn update_value_by_label(
        &mut self,
        label: &str,
        update: ResultSetUpdate,
    ) -> Result<(), RdbcError> {
        let call = match update {
            ResultSetUpdate::Ref(value) => {
                StrongGetterCall::UpdateRefLabel(label.to_owned(), value.is_some())
            }
            ResultSetUpdate::Blob(value) => {
                StrongGetterCall::UpdateBlobLabel(label.to_owned(), value.is_some())
            }
            ResultSetUpdate::Clob(value) => {
                StrongGetterCall::UpdateClobLabel(label.to_owned(), value.is_some())
            }
            ResultSetUpdate::Array(value) => {
                StrongGetterCall::UpdateArrayLabel(label.to_owned(), value.is_some())
            }
            ResultSetUpdate::RowId(value) => {
                StrongGetterCall::UpdateRowIdLabel(label.to_owned(), value.is_some())
            }
            ResultSetUpdate::NClob(value) => {
                StrongGetterCall::UpdateNClobLabel(label.to_owned(), value.is_some())
            }
            ResultSetUpdate::SqlXml(value) => {
                StrongGetterCall::UpdateSqlXmlLabel(label.to_owned(), value.is_some())
            }
            ResultSetUpdate::BlobStream { stream, length } => {
                StrongGetterCall::UpdateBlobStreamLabel(label.to_owned(), stream.is_some(), length)
            }
            ResultSetUpdate::ClobReader { reader, length } => {
                StrongGetterCall::UpdateClobReaderLabel(label.to_owned(), reader.is_some(), length)
            }
            ResultSetUpdate::NClobReader { reader, length } => {
                StrongGetterCall::UpdateNClobReaderLabel(label.to_owned(), reader.is_some(), length)
            }
            _ => {
                return Err(RdbcError::InvalidArgument(
                    "resource update identity lost".to_owned(),
                ))
            }
        };
        self.calls.lock().unwrap().push(call);
        Ok(())
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new())))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}
