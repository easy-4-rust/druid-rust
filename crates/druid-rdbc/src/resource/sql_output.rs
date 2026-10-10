use crate::error::RdbcError;
use crate::resource::SqlData;
use crate::resource::{
    RdbcArray, RdbcBlob, RdbcClob, RdbcInputStream, RdbcNClob, RdbcReader, RdbcRef, RdbcSqlXml,
    RdbcStruct,
};
use crate::types::{RdbcObject, RdbcRowId, RdbcUrl, SqlType};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::future::Future;
use std::pin::Pin;

/// 按SQL声明顺序写入UDT属性的独占标准输出合同。对应 Java: java.sql.SQLOutput。
/// 驱动调用SQLData.writeSQL，后者按字段声明顺序调用writer；NULL不得替换为空字符串或零。
#[async_trait::async_trait]
pub trait SqlOutput: Send {
    /// 写入参数value作为下一string属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeString。
    async fn write_string(&mut self, value: Option<String>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一boolean属性，返回成功或原数据库/转换错误。对应 Java: SQLOutput#writeBoolean。
    async fn write_boolean(&mut self, value: bool) -> Result<(), RdbcError>;
    /// 写入参数value作为下一byte属性，返回成功或原数据库/转换错误。对应 Java: SQLOutput#writeByte。
    async fn write_byte(&mut self, value: i8) -> Result<(), RdbcError>;
    /// 写入参数value作为下一short属性，返回成功或原数据库/转换错误。对应 Java: SQLOutput#writeShort。
    async fn write_short(&mut self, value: i16) -> Result<(), RdbcError>;
    /// 写入参数value作为下一int属性，返回成功或原数据库/转换错误。对应 Java: SQLOutput#writeInt。
    async fn write_int(&mut self, value: i32) -> Result<(), RdbcError>;
    /// 写入参数value作为下一long属性，返回成功或原数据库/转换错误。对应 Java: SQLOutput#writeLong。
    async fn write_long(&mut self, value: i64) -> Result<(), RdbcError>;
    /// 写入参数value作为下一float属性，返回成功或原数据库/转换错误。对应 Java: SQLOutput#writeFloat。
    async fn write_float(&mut self, value: f32) -> Result<(), RdbcError>;
    /// 写入参数value作为下一double属性，返回成功或原数据库/转换错误。对应 Java: SQLOutput#writeDouble。
    async fn write_double(&mut self, value: f64) -> Result<(), RdbcError>;
    /// 写入参数value作为下一big_decimal属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeBigDecimal。
    async fn write_big_decimal(&mut self, value: Option<BigDecimal>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一bytes属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeBytes。
    async fn write_bytes(&mut self, value: Option<Vec<u8>>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一date属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeDate。
    async fn write_date(&mut self, value: Option<NaiveDate>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一time属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeTime。
    async fn write_time(&mut self, value: Option<NaiveTime>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一timestamp属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeTimestamp。
    async fn write_timestamp(&mut self, value: Option<NaiveDateTime>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一character_stream属性，返回成功或原数据库/转换错误。None写入SQL NULL。保留原流身份及当前位置，不提前读完。对应 Java: SQLOutput#writeCharacterStream。
    async fn write_character_stream(&mut self, value: Option<RdbcReader>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一ascii_stream属性，返回成功或原数据库/转换错误。None写入SQL NULL。保留原流身份及当前位置，不提前读完。对应 Java: SQLOutput#writeAsciiStream。
    async fn write_ascii_stream(&mut self, value: Option<RdbcInputStream>)
        -> Result<(), RdbcError>;
    /// 写入参数value作为下一binary_stream属性，返回成功或原数据库/转换错误。None写入SQL NULL。保留原流身份及当前位置，不提前读完。对应 Java: SQLOutput#writeBinaryStream。
    async fn write_binary_stream(
        &mut self,
        value: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError>;
    /// 写入参数value作为下一object属性，返回成功或原数据库/转换错误。None写入SQL NULL。非NULL对象调用其writeSQL，按其SQL类型保存一个自定义类型属性边界；错误原样传播。对应 Java: SQLOutput#writeObject。
    async fn write_object(&mut self, value: Option<&mut dyn SqlData>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一ref属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeRef。
    async fn write_ref(&mut self, value: Option<RdbcRef>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一blob属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeBlob。
    async fn write_blob(&mut self, value: Option<RdbcBlob>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一clob属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeClob。
    async fn write_clob(&mut self, value: Option<RdbcClob>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一struct属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeStruct。
    async fn write_struct(&mut self, value: Option<RdbcStruct>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一array属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeArray。
    async fn write_array(&mut self, value: Option<RdbcArray>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一url属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeURL。
    async fn write_url(&mut self, value: Option<RdbcUrl>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一n_clob属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeNClob。
    async fn write_n_clob(&mut self, value: Option<RdbcNClob>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一n_string属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeNString。
    async fn write_n_string(&mut self, value: Option<String>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一sql_xml属性，返回成功或原数据库/转换错误。None写入SQL NULL。SQLXML的Writer/OutputStream/Result尚未关闭或XML处理失败时，驱动必须返回SQL异常。对应 Java: SQLOutput#writeSQLXML。
    async fn write_sql_xml(&mut self, value: Option<RdbcSqlXml>) -> Result<(), RdbcError>;
    /// 写入参数value作为下一row_id属性，返回成功或原数据库/转换错误。None写入SQL NULL。对应 Java: SQLOutput#writeRowId。
    async fn write_row_id(&mut self, value: Option<RdbcRowId>) -> Result<(), RdbcError>;
    /// 将参数value转换为参数target的SQL类型并写入，返回成功或转换/数据库错误。
    /// NULL写SQL NULL；SQLData须调用writeSQL，资源须按对应SQL类型传递。默认不追加属性并报告不支持。
    /// 返回Future仅借用self；覆盖实现应在调用时读取所需描述符，不将非Sync引用移入Send Future。对应 Java: SQLOutput#writeObject(Object, SQLType)。
    fn write_object_as<'a>(
        &'a mut self,
        _value: Option<RdbcObject>,
        _target: &dyn SqlType,
    ) -> Pin<Box<dyn Future<Output = Result<(), RdbcError>> + Send + 'a>> {
        // Java默认方法同步报告不支持；返回Future不保存也不要求描述符Sync。
        Box::pin(async {
            Err(RdbcError::feature_not_supported(
                "SQLOutput#writeObject(Object, SQLType)",
            ))
        })
    }
}
