use crate::error::RdbcError;
use crate::extensions::{
    buffered_sql_struct::BufferedSqlStruct, sql_attribute::SqlAttribute, BufferedSqlInput,
};
use crate::resource::{
    RdbcArray, RdbcBlob, RdbcClob, RdbcInputStream, RdbcNClob, RdbcReader, RdbcRef, RdbcSqlXml,
    RdbcStruct,
};
use crate::resource::{SqlData, SqlOutput};
use crate::types::{RdbcObject, RdbcRowId, RdbcUrl, Value};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// 按声明顺序保存UDT属性的标准输出实现，保留NULL、资源与流引用。对应 Java: java.sql.SQLOutput（Rust缓冲扩展）。
#[derive(Default)]
pub struct BufferedSqlOutput {
    attributes: Vec<SqlAttribute>,
}
impl BufferedSqlOutput {
    /// 创建空属性输出，无参数，返回独占writer。
    pub fn new() -> Self {
        Self::default()
    }
    /// 将本输出消费为输入，无参数，返回保留属性顺序和流身份的reader。
    pub fn into_input(self) -> BufferedSqlInput {
        BufferedSqlInput::from_attributes(self.attributes)
    }
    /// 追加一个原标准对象，不读取或释放其资源。
    fn push(&mut self, value: RdbcObject) {
        self.attributes.push(SqlAttribute::from_object(value));
    }
}
#[async_trait::async_trait]
impl SqlOutput for BufferedSqlOutput {
    async fn write_boolean(&mut self, value: bool) -> Result<(), RdbcError> {
        self.push(RdbcObject::Scalar(Value::Bool(value)));
        Ok(())
    }
    async fn write_byte(&mut self, value: i8) -> Result<(), RdbcError> {
        self.push(RdbcObject::Scalar(Value::Int(i64::from(value))));
        Ok(())
    }
    async fn write_short(&mut self, value: i16) -> Result<(), RdbcError> {
        self.push(RdbcObject::Scalar(Value::Int(i64::from(value))));
        Ok(())
    }
    async fn write_int(&mut self, value: i32) -> Result<(), RdbcError> {
        self.push(RdbcObject::Scalar(Value::Int(i64::from(value))));
        Ok(())
    }
    async fn write_long(&mut self, value: i64) -> Result<(), RdbcError> {
        self.push(RdbcObject::Scalar(Value::Int(value)));
        Ok(())
    }
    async fn write_float(&mut self, value: f32) -> Result<(), RdbcError> {
        self.push(RdbcObject::Scalar(Value::Float(f64::from(value))));
        Ok(())
    }
    async fn write_double(&mut self, value: f64) -> Result<(), RdbcError> {
        self.push(RdbcObject::Scalar(Value::Float(value)));
        Ok(())
    }
    async fn write_string(&mut self, value: Option<String>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), |v| {
            RdbcObject::Scalar(Value::String(v))
        }));
        Ok(())
    }
    async fn write_big_decimal(&mut self, value: Option<BigDecimal>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), |v| {
            RdbcObject::Scalar(Value::Decimal(v))
        }));
        Ok(())
    }
    async fn write_bytes(&mut self, value: Option<Vec<u8>>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), |v| {
            RdbcObject::Scalar(Value::Bytes(v))
        }));
        Ok(())
    }
    async fn write_date(&mut self, value: Option<NaiveDate>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), |v| {
            RdbcObject::Scalar(Value::Date(v))
        }));
        Ok(())
    }
    async fn write_time(&mut self, value: Option<NaiveTime>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), |v| {
            RdbcObject::Scalar(Value::Time(v))
        }));
        Ok(())
    }
    async fn write_timestamp(&mut self, value: Option<NaiveDateTime>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), |v| {
            RdbcObject::Scalar(Value::Timestamp(v))
        }));
        Ok(())
    }
    async fn write_n_string(&mut self, value: Option<String>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::NString));
        Ok(())
    }
    async fn write_character_stream(&mut self, value: Option<RdbcReader>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::CharacterStream));
        Ok(())
    }
    async fn write_ref(&mut self, value: Option<RdbcRef>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Ref));
        Ok(())
    }
    async fn write_blob(&mut self, value: Option<RdbcBlob>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Blob));
        Ok(())
    }
    async fn write_clob(&mut self, value: Option<RdbcClob>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Clob));
        Ok(())
    }
    async fn write_array(&mut self, value: Option<RdbcArray>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Array));
        Ok(())
    }
    async fn write_url(&mut self, value: Option<RdbcUrl>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Url));
        Ok(())
    }
    async fn write_n_clob(&mut self, value: Option<RdbcNClob>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::NClob));
        Ok(())
    }
    async fn write_sql_xml(&mut self, value: Option<RdbcSqlXml>) -> Result<(), RdbcError> {
        // 驱动确认写入端已关闭且XML可用后才发布属性；错误或取消不留下部分输出。
        if let Some(value) = &value {
            value.validate_for_binding().await?;
        }
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::SqlXml));
        Ok(())
    }
    async fn write_row_id(&mut self, value: Option<RdbcRowId>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::RowId));
        Ok(())
    }
    async fn write_struct(&mut self, value: Option<RdbcStruct>) -> Result<(), RdbcError> {
        self.push(value.map_or(RdbcObject::Scalar(Value::Null), RdbcObject::Struct));
        Ok(())
    }
    async fn write_ascii_stream(
        &mut self,
        value: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError> {
        self.attributes.push(value.map_or(
            SqlAttribute::Object(RdbcObject::Scalar(Value::Null)),
            SqlAttribute::AsciiStream,
        ));
        Ok(())
    }
    async fn write_binary_stream(
        &mut self,
        value: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError> {
        self.attributes.push(value.map_or(
            SqlAttribute::Object(RdbcObject::Scalar(Value::Null)),
            SqlAttribute::BinaryStream,
        ));
        Ok(())
    }
    async fn write_object(&mut self, value: Option<&mut dyn SqlData>) -> Result<(), RdbcError> {
        let Some(value) = value else {
            self.push(RdbcObject::Scalar(Value::Null));
            return Ok(());
        };
        // 子输出成功后才发布一个结构属性，避免失败泄漏半个UDT到外层。
        let type_name = value.sql_type_name().await?;
        let mut child = Self::new();
        value.write_sql(&mut child).await?;
        let attributes = child
            .attributes
            .into_iter()
            .map(SqlAttribute::into_object)
            .collect();
        self.push(RdbcObject::Struct(RdbcStruct::from_standard(Box::new(
            BufferedSqlStruct {
                type_name,
                attributes,
            },
        ))));
        Ok(())
    }
}
