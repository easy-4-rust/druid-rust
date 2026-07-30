//! 对应 Java：`com.alibaba.druid.filter.encoding.EncodingConvertFilter`。

use super::CharsetConvert;
use crate::core::{
    AfterFilter, BeforeFilter, ClobFilterChain, DruidError, ExecContext, ExecResult, JavaString,
    JdbcObject, JdbcReader, ResultSetFilter, ResultSetFilterChain, Value,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::Duration;

/// JDBC SQL、参数和结果字符编码转换 Filter。
///
/// 本对象实现 Java 的字符串/Reader 值转换以及 ResultSet around-chain。
/// SQL prepare/execute 入参的生产接线由连接/Statement FilterChain 继续承接，
/// 不能把此对象的存在误记为整个 `SEM-FLT-015` 已完成。
#[derive(Debug)]
pub struct EncodingConvertFilter {
    charset_convert: RwLock<CharsetConvert>,
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
            charset_convert: RwLock::new(CharsetConvert::new(client_encoding, server_encoding)?),
        })
    }

    /// 编码 SQL 或字符串参数。
    pub fn encode(&self, value: &str) -> Result<String, DruidError> {
        self.charset_convert.read().encode(value)
    }

    /// 解码字符串结果。
    pub fn decode(&self, value: &str) -> Result<String, DruidError> {
        self.charset_convert.read().decode(value)
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

    fn encode_java_string(&self, value: &JavaString) -> Result<JavaString, DruidError> {
        let value = String::from_utf16_lossy(value.as_utf16());
        self.encode(&value).map(JavaString::from)
    }

    fn decode_java_string(&self, value: JavaString) -> Result<JavaString, DruidError> {
        let value = String::from_utf16_lossy(value.as_utf16());
        self.decode(&value).map(JavaString::from)
    }
}

impl Clone for EncodingConvertFilter {
    fn clone(&self) -> Self {
        Self {
            charset_convert: RwLock::new(self.charset_convert.read().clone()),
        }
    }
}

#[async_trait::async_trait]
impl BeforeFilter for EncodingConvertFilter {
    fn name(&self) -> &str {
        "encoding"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        context.sql = self.encode(&context.sql)?;
        Ok(())
    }

    fn prepare_statement_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.encode(sql)
    }

    fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        self.encode(sql)
    }

    fn config_from_properties(
        &self,
        properties: &HashMap<String, String>,
    ) -> Result<(), DruidError> {
        let client = properties
            .get(Self::CLIENT_ENCODING_KEY)
            .map(String::as_str);
        let server = properties
            .get(Self::SERVER_ENCODING_KEY)
            .map(String::as_str);
        *self.charset_convert.write() = CharsetConvert::new(client, server)?;
        Ok(())
    }

    fn clob_position_string(
        &self,
        chain: &mut ClobFilterChain<'_>,
        pattern: &JavaString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        let pattern = self.encode_java_string(pattern)?;
        chain.clob_position_string(&pattern, start)
    }

    fn clob_get_sub_string(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
        length: i32,
    ) -> Result<JavaString, DruidError> {
        self.decode_java_string(chain.clob_get_sub_string(position, length)?)
    }

    fn clob_get_character_stream(
        &self,
        chain: &mut ClobFilterChain<'_>,
    ) -> Result<JdbcReader, DruidError> {
        let text = chain.clob_get_character_stream()?.read_to_string()?;
        self.decode(&text).map(JdbcReader::from_string)
    }

    fn clob_get_character_stream_range(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
        length: i64,
    ) -> Result<JdbcReader, DruidError> {
        let text = chain
            .clob_get_character_stream_range(position, length)?
            .read_to_string()?;
        self.decode(&text).map(JdbcReader::from_string)
    }

    fn clob_set_string(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
        value: &JavaString,
    ) -> Result<i32, DruidError> {
        let value = self.encode_java_string(value)?;
        chain.clob_set_string(position, &value)
    }

    fn clob_set_string_range(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        let value = self.encode_java_string(value)?;
        chain.clob_set_string_range(position, &value, offset, length)
    }
}

#[async_trait::async_trait]
impl AfterFilter for EncodingConvertFilter {
    fn name(&self) -> &str {
        "encoding"
    }

    async fn after(
        &self,
        _context: &ExecContext<'_>,
        _result: &Result<ExecResult, DruidError>,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
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
