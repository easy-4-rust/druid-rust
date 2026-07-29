//! 对应 Java：`com.alibaba.druid.filter.encoding.EncodingConvertFilter`。

use super::CharsetConvert;
use crate::core::{
    DruidError, JdbcObject, JdbcReader, ResultSetFilter, ResultSetFilterChain, Value,
};

/// JDBC SQL、参数和结果字符编码转换 Filter。
///
/// 本对象实现 Java 的字符串/Reader 值转换以及 ResultSet around-chain。
/// SQL prepare/execute 入参的生产接线由连接/Statement FilterChain 继续承接，
/// 不能把此对象的存在误记为整个 `SEM-FLT-015` 已完成。
#[derive(Debug, Clone)]
pub struct EncodingConvertFilter {
    charset_convert: CharsetConvert,
}

impl EncodingConvertFilter {
    /// Java 连接属性中保存转换器的键。
    pub const ATTR_CHARSET_CONVERTER: &'static str = "ali.charset.converter";
    /// 客户端编码配置键。
    pub const CLIENT_ENCODING_KEY: &'static str = "clientEncoding";
    /// 服务端编码配置键。
    pub const SERVER_ENCODING_KEY: &'static str = "serverEncoding";

    /// 创建固定于一个物理连接配置的转换 Filter。
    pub fn new(
        client_encoding: Option<&str>,
        server_encoding: Option<&str>,
    ) -> Result<Self, DruidError> {
        Ok(Self {
            charset_convert: CharsetConvert::new(client_encoding, server_encoding)?,
        })
    }

    /// 编码 SQL 或字符串参数。
    pub fn encode(&self, value: &str) -> Result<String, DruidError> {
        self.charset_convert.encode(value)
    }

    /// 解码字符串结果。
    pub fn decode(&self, value: &str) -> Result<String, DruidError> {
        self.charset_convert.decode(value)
    }

    /// 编码 JDBC 标量参数；非字符串值原样返回。
    pub fn encode_value(&self, value: Value) -> Result<Value, DruidError> {
        match value {
            Value::String(value) => self.encode(&value).map(Value::String),
            value => Ok(value),
        }
    }

    /// 解码 JDBC 标量结果；非字符串值原样返回。
    pub fn decode_value(&self, value: Value) -> Result<Value, DruidError> {
        match value {
            Value::String(value) => self.decode(&value).map(Value::String),
            value => Ok(value),
        }
    }

    fn decode_object(&self, object: JdbcObject) -> Result<JdbcObject, DruidError> {
        match object {
            JdbcObject::Scalar(value) => self.decode_value(value).map(JdbcObject::Scalar),
            JdbcObject::String(value) => self.decode(&value).map(JdbcObject::String),
            JdbcObject::NString(value) => self.decode(&value).map(JdbcObject::NString),
            JdbcObject::CharacterStream(reader) => {
                let value = reader.read_to_string()?;
                self.decode(&value)
                    .map(JdbcReader::from_string)
                    .map(JdbcObject::CharacterStream)
            }
            JdbcObject::NCharacterStream(reader) => {
                let value = reader.read_to_string()?;
                self.decode(&value)
                    .map(JdbcReader::from_string)
                    .map(JdbcObject::NCharacterStream)
            }
            object => Ok(object),
        }
    }
}

impl ResultSetFilter for EncodingConvertFilter {
    fn result_set_get_string(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Option<String>, DruidError> {
        chain
            .result_set_get_string(column_index)?
            .map(|value| self.decode(&value))
            .transpose()
    }

    fn result_set_get_string_by_label(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Option<String>, DruidError> {
        chain
            .result_set_get_string_by_label(column_label)?
            .map(|value| self.decode(&value))
            .transpose()
    }

    fn result_set_get_object(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
    ) -> Result<Value, DruidError> {
        self.decode_value(chain.result_set_get_object(column_index)?)
    }

    fn result_set_get_object_by_label(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
    ) -> Result<Value, DruidError> {
        self.decode_value(chain.result_set_get_object_by_label(column_label)?)
    }

    fn result_set_get_object_with_type_map(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_index: usize,
        type_map: Option<&crate::core::JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        self.decode_object(chain.result_set_get_object_with_type_map(column_index, type_map)?)
    }

    fn result_set_get_object_by_label_with_type_map(
        &self,
        chain: &mut ResultSetFilterChain<'_>,
        column_label: &str,
        type_map: Option<&crate::core::JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        self.decode_object(
            chain.result_set_get_object_by_label_with_type_map(column_label, type_map)?,
        )
    }
}
