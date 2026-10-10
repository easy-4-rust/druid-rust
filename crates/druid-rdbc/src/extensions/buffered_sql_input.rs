use crate::error::{RdbcError, SqlException};
use crate::extensions::sql_attribute::SqlAttribute;
use crate::extensions::SqlDataRegistry;
use crate::resource::SqlInput;
use crate::resource::{
    RdbcArray, RdbcBlob, RdbcClob, RdbcInputStream, RdbcNClob, RdbcReader, RdbcRef, RdbcSqlXml,
};
use crate::types::RdbcTypeMap;
use crate::types::{RdbcObject, RdbcRowId, RdbcUrl, Value};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::sync::Arc;

/// UDT属性缓冲输入；仅通过驱动显式提供的映射和工厂构造应用类型。对应 Java: java.sql.SQLInput（Rust缓冲扩展）。
pub struct BufferedSqlInput {
    values: std::vec::IntoIter<SqlAttribute>,
    was_null: bool,
    type_mapping: Option<(RdbcTypeMap, Arc<SqlDataRegistry>)>,
}
impl BufferedSqlInput {
    /// 使用参数values按声明顺序创建输入；保留标准对象与流身份，返回独占reader。
    pub fn new(values: Vec<RdbcObject>) -> Self {
        Self::from_attributes(values.into_iter().map(SqlAttribute::from_object).collect())
    }
    /// 使用参数values、type_map和registry创建自定义映射输入；返回声明顺序reader。
    /// 映射由驱动在调用应用readSQL前安装，嵌套输入继承同一目录。对应 Java: SQLInput#readObject。
    pub fn with_type_map(
        values: Vec<RdbcObject>,
        type_map: RdbcTypeMap,
        registry: Arc<SqlDataRegistry>,
    ) -> Self {
        let mut input = Self::new(values);
        input.type_mapping = Some((type_map, registry));
        input
    }
    /// 将同一标准输出的内部属性交接给reader，保持所有流标签。
    pub(crate) fn from_attributes(values: Vec<SqlAttribute>) -> Self {
        Self {
            values: values.into_iter(),
            was_null: false,
            type_mapping: None,
        }
    }
    /// 取得并仅消费下一属性；耗尽返回SQL错误，不修改上次NULL状态。
    fn next_attribute(&mut self) -> Result<SqlAttribute, RdbcError> {
        let value = self.values.next().ok_or_else(|| {
            RdbcError::SqlException(Box::new(SqlException::new(
                0,
                Some("02000".into()),
                Some("SQLInput has no remaining attributes".into()),
            )))
        })?;
        self.was_null = value.is_null();
        Ok(value)
    }
    /// 返回类型转换错误，保留SQLState而不泄漏属性值。
    fn conversion_error() -> RdbcError {
        RdbcError::SqlException(Box::new(
            SqlException::new(
                0,
                Some("22005".into()),
                Some("SQLInput attribute has an incompatible type".into()),
            )
            .with_class_name("java.sql.SQLDataException"),
        ))
    }
    /// 取得下一标量并保留已有typed标量的数值、时间和decimal精度。
    fn next_scalar(&mut self) -> Result<Value, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(v) => Ok(v),
            RdbcObject::String(v) | RdbcObject::NString(v) => Ok(Value::String(v)),
            RdbcObject::Boolean(v) => Ok(Value::Bool(v)),
            RdbcObject::Byte(v) => Ok(Value::Int(i64::from(v))),
            RdbcObject::Short(v) => Ok(Value::Int(i64::from(v))),
            RdbcObject::Integer(v) => Ok(Value::Int(i64::from(v))),
            RdbcObject::Long(v) => Ok(Value::Int(v)),
            RdbcObject::Float(v) => Ok(Value::Float(f64::from(v))),
            RdbcObject::Double(v) => Ok(Value::Float(v)),
            RdbcObject::Bytes(v) => Ok(Value::Bytes(v)),
            RdbcObject::BigDecimal(v) => Ok(Value::Decimal(v)),
            RdbcObject::Date(v) => Ok(Value::Date(v)),
            RdbcObject::Time(v) => Ok(Value::Time(v)),
            RdbcObject::Timestamp(v) => Ok(Value::Timestamp(v)),
            _ => Err(Self::conversion_error()),
        }
    }
}
#[async_trait::async_trait]
impl SqlInput for BufferedSqlInput {
    fn was_null(&self) -> Result<bool, RdbcError> {
        Ok(self.was_null)
    }
    async fn read_string(&mut self) -> Result<Option<String>, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(None),
            Value::String(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_big_decimal(&mut self) -> Result<Option<BigDecimal>, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(None),
            Value::Decimal(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_bytes(&mut self) -> Result<Option<Vec<u8>>, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(None),
            Value::Bytes(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_date(&mut self) -> Result<Option<NaiveDate>, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(None),
            Value::Date(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_time(&mut self) -> Result<Option<NaiveTime>, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(None),
            Value::Time(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_timestamp(&mut self) -> Result<Option<NaiveDateTime>, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(None),
            Value::Timestamp(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_n_string(&mut self) -> Result<Option<String>, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(None),
            Value::String(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_boolean(&mut self) -> Result<bool, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(false),
            Value::Bool(v) => Ok(v),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_long(&mut self) -> Result<i64, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(0),
            Value::Int(v) => Ok(v),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_double(&mut self) -> Result<f64, RdbcError> {
        match self.next_scalar()? {
            Value::Null => Ok(0.0),
            Value::Float(v) => Ok(v),
            Value::Int(v) => Ok(v as f64),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_float(&mut self) -> Result<f32, RdbcError> {
        Ok(self.read_double().await? as f32)
    }
    async fn read_byte(&mut self) -> Result<i8, RdbcError> {
        i8::try_from(self.read_long().await?).map_err(|_| Self::conversion_error())
    }
    async fn read_short(&mut self) -> Result<i16, RdbcError> {
        i16::try_from(self.read_long().await?).map_err(|_| Self::conversion_error())
    }
    async fn read_int(&mut self) -> Result<i32, RdbcError> {
        i32::try_from(self.read_long().await?).map_err(|_| Self::conversion_error())
    }
    async fn read_character_stream(&mut self) -> Result<Option<RdbcReader>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::CharacterStream(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_ref(&mut self) -> Result<Option<RdbcRef>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Ref(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_blob(&mut self) -> Result<Option<RdbcBlob>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Blob(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_clob(&mut self) -> Result<Option<RdbcClob>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Clob(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_array(&mut self) -> Result<Option<RdbcArray>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Array(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_url(&mut self) -> Result<Option<RdbcUrl>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Url(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_n_clob(&mut self) -> Result<Option<RdbcNClob>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::NClob(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_sql_xml(&mut self) -> Result<Option<RdbcSqlXml>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::SqlXml(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_row_id(&mut self) -> Result<Option<RdbcRowId>, RdbcError> {
        match self.next_attribute()?.into_object() {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::RowId(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_ascii_stream(&mut self) -> Result<Option<RdbcInputStream>, RdbcError> {
        match self.next_attribute()? {
            SqlAttribute::Object(RdbcObject::Scalar(Value::Null)) => Ok(None),
            SqlAttribute::AsciiStream(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_binary_stream(&mut self) -> Result<Option<RdbcInputStream>, RdbcError> {
        match self.next_attribute()? {
            SqlAttribute::Object(RdbcObject::Scalar(Value::Null)) => Ok(None),
            SqlAttribute::BinaryStream(v) => Ok(Some(v)),
            _ => Err(Self::conversion_error()),
        }
    }
    async fn read_object(&mut self) -> Result<RdbcObject, RdbcError> {
        let value = self.next_attribute()?.into_object();
        match &self.type_mapping {
            Some((map, registry)) => registry.map_object(value, map).await,
            None => Ok(value),
        }
    }
}
