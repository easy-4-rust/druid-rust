use crate::error::RdbcError;
use crate::resource::{
    RdbcArray, RdbcBlob, RdbcClob, RdbcInputStream, RdbcNClob, RdbcReader, RdbcRef, RdbcSqlXml,
};
use crate::types::{RdbcObject, RdbcRowId, RdbcTargetType, RdbcUrl};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// 按SQL声明顺序读取UDT属性的独占标准输入合同。对应 Java: java.sql.SQLInput。
/// 驱动初始化类型映射后交给SQLData；每次reader读取下一属性，wasNull保留最近读取的NULL状态。
/// 读取数据库、映射或转换失败返回原错误；不支持的方法必须报告SQLFeatureNotSupportedException。
#[async_trait::async_trait]
pub trait SqlInput: Send {
    /// 读取下一string属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readString。
    async fn read_string(&mut self) -> Result<Option<String>, RdbcError>;
    /// 读取下一boolean属性，无参数，返回属性或数据库/转换错误。SQL NULL返回零或false，并由was_null区分。对应 Java: SQLInput#readBoolean。
    async fn read_boolean(&mut self) -> Result<bool, RdbcError>;
    /// 读取下一byte属性，无参数，返回属性或数据库/转换错误。SQL NULL返回零或false，并由was_null区分。对应 Java: SQLInput#readByte。
    async fn read_byte(&mut self) -> Result<i8, RdbcError>;
    /// 读取下一short属性，无参数，返回属性或数据库/转换错误。SQL NULL返回零或false，并由was_null区分。对应 Java: SQLInput#readShort。
    async fn read_short(&mut self) -> Result<i16, RdbcError>;
    /// 读取下一int属性，无参数，返回属性或数据库/转换错误。SQL NULL返回零或false，并由was_null区分。对应 Java: SQLInput#readInt。
    async fn read_int(&mut self) -> Result<i32, RdbcError>;
    /// 读取下一long属性，无参数，返回属性或数据库/转换错误。SQL NULL返回零或false，并由was_null区分。对应 Java: SQLInput#readLong。
    async fn read_long(&mut self) -> Result<i64, RdbcError>;
    /// 读取下一float属性，无参数，返回属性或数据库/转换错误。SQL NULL返回零或false，并由was_null区分。对应 Java: SQLInput#readFloat。
    async fn read_float(&mut self) -> Result<f32, RdbcError>;
    /// 读取下一double属性，无参数，返回属性或数据库/转换错误。SQL NULL返回零或false，并由was_null区分。对应 Java: SQLInput#readDouble。
    async fn read_double(&mut self) -> Result<f64, RdbcError>;
    /// 读取下一big_decimal属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readBigDecimal。
    async fn read_big_decimal(&mut self) -> Result<Option<BigDecimal>, RdbcError>;
    /// 读取下一bytes属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readBytes。
    async fn read_bytes(&mut self) -> Result<Option<Vec<u8>>, RdbcError>;
    /// 读取下一date属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readDate。
    async fn read_date(&mut self) -> Result<Option<NaiveDate>, RdbcError>;
    /// 读取下一time属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readTime。
    async fn read_time(&mut self) -> Result<Option<NaiveTime>, RdbcError>;
    /// 读取下一timestamp属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readTimestamp。
    async fn read_timestamp(&mut self) -> Result<Option<NaiveDateTime>, RdbcError>;
    /// 读取下一character_stream属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。保留原流身份及当前位置，不提前物化。对应 Java: SQLInput#readCharacterStream。
    async fn read_character_stream(&mut self) -> Result<Option<RdbcReader>, RdbcError>;
    /// 读取下一ascii_stream属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。保留原流身份及当前位置，不提前物化。对应 Java: SQLInput#readAsciiStream。
    async fn read_ascii_stream(&mut self) -> Result<Option<RdbcInputStream>, RdbcError>;
    /// 读取下一binary_stream属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。保留原流身份及当前位置，不提前物化。对应 Java: SQLInput#readBinaryStream。
    async fn read_binary_stream(&mut self) -> Result<Option<RdbcInputStream>, RdbcError>;
    /// 读取下一object属性，无参数，返回属性或数据库/转换错误。SQL NULL返回RdbcObject::Scalar(Value::Null)；驱动按默认映射和已注册type map构造自定义SQLData，并调用其readSQL。对应 Java: SQLInput#readObject。
    async fn read_object(&mut self) -> Result<RdbcObject, RdbcError>;
    /// 读取下一ref属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readRef。
    async fn read_ref(&mut self) -> Result<Option<RdbcRef>, RdbcError>;
    /// 读取下一blob属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readBlob。
    async fn read_blob(&mut self) -> Result<Option<RdbcBlob>, RdbcError>;
    /// 读取下一clob属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readClob。
    async fn read_clob(&mut self) -> Result<Option<RdbcClob>, RdbcError>;
    /// 读取下一array属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readArray。
    async fn read_array(&mut self) -> Result<Option<RdbcArray>, RdbcError>;
    /// 读取下一url属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readURL。
    async fn read_url(&mut self) -> Result<Option<RdbcUrl>, RdbcError>;
    /// 读取下一n_clob属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readNClob。
    async fn read_n_clob(&mut self) -> Result<Option<RdbcNClob>, RdbcError>;
    /// 读取下一n_string属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readNString。
    async fn read_n_string(&mut self) -> Result<Option<String>, RdbcError>;
    /// 读取下一sql_xml属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readSQLXML。
    async fn read_sql_xml(&mut self) -> Result<Option<RdbcSqlXml>, RdbcError>;
    /// 读取下一row_id属性，无参数，返回属性或数据库/转换错误。SQL NULL返回None。对应 Java: SQLInput#readRowId。
    async fn read_row_id(&mut self) -> Result<Option<RdbcRowId>, RdbcError>;
    /// 返回最后读取的属性是否SQL NULL，无参数；驱动访问失败返回错误。对应 Java: SQLInput#wasNull。
    fn was_null(&self) -> Result<bool, RdbcError>;
    /// 按参数target要求转换下一属性，返回对象或NULL；驱动覆盖时必须应用type map和自定义SQLData协议。
    /// 默认不消费属性并报告不支持。对应 Java: SQLInput#readObject(Class)。
    async fn read_object_as(&mut self, _target: &RdbcTargetType) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "SQLInput#readObject(Class)",
        ))
    }
}
