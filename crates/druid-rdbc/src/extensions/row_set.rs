use super::RowSetListener;
use crate::connection::TransactionIsolation;
use crate::error::RdbcError;
use crate::result_set::ResultSet;
use crate::statement::{ParameterIndex, PreparedInputParameter, PreparedTypeNameArgument};
use crate::types::{
    RdbcArray, RdbcBigDecimal, RdbcBlob, RdbcCalendar, RdbcCalendarArgument, RdbcCharacterLength,
    RdbcClob, RdbcInputStream, RdbcNClob, RdbcNaiveDate, RdbcNaiveDateTime, RdbcNaiveTime,
    RdbcObject, RdbcReader, RdbcRef, RdbcRowId, RdbcSqlXml, RdbcStreamLength, RdbcTypeMap, RdbcUrl,
};
use std::sync::{Arc, Mutex};

/// 继承唯一标准 ResultSet 的行集接口，完整保存连接属性及 JDBC 参数重载。
/// 对应 Java: javax.sql.RowSet；不继承 Statement/PreparedStatement。
#[async_trait::async_trait]
pub trait RowSet: ResultSet {
    /// 按参数 index 绑定无损 setter 描述符；返回成功或 SQL 状态错误，不消费流。
    fn set_parameter(
        &mut self,
        index: ParameterIndex,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError>;
    /// 绑定原始参数 name 的描述符；返回成功或 SQL 错误，最终位置由 provider 解析。
    fn set_named_parameter(
        &mut self,
        name: Option<&str>,
        parameter: PreparedInputParameter,
    ) -> Result<(), RdbcError>;
    /// 清除全部参数引用；无参数，不隐式关闭调用者流。对应 Java: RowSet#clearParameters。
    fn clear_parameters(&mut self) -> Result<(), RdbcError>;
    /// 执行当前命令并原子发布完整结果；无参数，保留 provider 原错误。对应 Java: RowSet#execute。
    async fn execute(&mut self) -> Result<(), RdbcError>;
    /// 登记参数 listener 的同一身份；无返回值。对应 Java: RowSet#addRowSetListener。
    fn add_row_set_listener(&mut self, listener: Arc<dyn RowSetListener>);
    /// 移除参数 listener 的登记身份；无返回值。对应 Java: RowSet#removeRowSetListener。
    fn remove_row_set_listener(&mut self, listener: &Arc<dyn RowSetListener>);
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getUrl。
    fn url(&self) -> Option<String>;
    /// 使用参数 url 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setUrl。
    fn set_url(&mut self, url: Option<&str>) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getDataSourceName。
    fn data_source_name(&self) -> Option<String>;
    /// 使用参数 name 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setDataSourceName。
    fn set_data_source_name(&mut self, name: Option<&str>) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getUsername。
    fn username(&self) -> Option<String>;
    /// 使用参数 name 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setUsername。
    fn set_username(&mut self, name: Option<&str>) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getPassword。
    fn password(&self) -> Option<String>;
    /// 使用参数 password 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setPassword。
    fn set_password(&mut self, password: Option<&str>) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getTransactionIsolation。
    fn transaction_isolation(&self) -> TransactionIsolation;
    /// 使用参数 level 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setTransactionIsolation。
    fn set_transaction_isolation(&mut self, level: i32) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getTypeMap。
    fn type_map(&self) -> Arc<Mutex<RdbcTypeMap>>;
    /// 使用参数 map 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setTypeMap。
    fn set_type_map(&mut self, map: Arc<Mutex<RdbcTypeMap>>) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getCommand。
    fn command(&self) -> Option<String>;
    /// 使用参数 command 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setCommand。
    fn set_command(&mut self, command: Option<&str>) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#isReadOnly。
    fn is_read_only(&self) -> bool;
    /// 使用参数 value 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setReadOnly。
    fn set_read_only(&mut self, value: bool) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getMaxFieldSize。
    fn max_field_size(&self) -> i32;
    /// 使用参数 max 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setMaxFieldSize。
    fn set_max_field_size(&mut self, max: i32) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getMaxRows。
    fn max_rows(&self) -> i32;
    /// 使用参数 max 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setMaxRows。
    fn set_max_rows(&mut self, max: i32) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getEscapeProcessing。
    fn escape_processing(&self) -> bool;
    /// 使用参数 enable 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setEscapeProcessing。
    fn set_escape_processing(&mut self, enable: bool) -> Result<(), RdbcError>;
    /// 获取当前属性，无参数，返回原请求值。对应 Java: RowSet#getQueryTimeout。
    fn query_timeout(&self) -> i32;
    /// 使用参数 seconds 设置请求属性，返回成功或原校验错误。对应 Java: RowSet#setQueryTimeout。
    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError>;
    /// 设置参数 type_code 的 JDBC 游标类型，返回成功或 SQL 错误。对应 Java: RowSet#setType。
    fn set_type(&mut self, type_code: i32) -> Result<(), RdbcError>;
    /// 设置参数 concurrency 的 JDBC 并发模式，返回成功或 SQL 错误。对应 Java: RowSet#setConcurrency。
    fn set_concurrency(&mut self, concurrency: i32) -> Result<(), RdbcError>;
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNull(int parameterIndex, int sqlType) throws SQLException;
    fn set_null(&mut self, index: ParameterIndex, sql_type: i32) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Null {
                sql_type,
                type_name: PreparedTypeNameArgument::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNull(String parameterName, int sqlType) throws SQLException;
    fn set_null_by_name(&mut self, name: Option<&str>, sql_type: i32) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Null {
                sql_type,
                type_name: PreparedTypeNameArgument::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNull (int paramIndex, int sqlType, String typeName) throws SQLException;
    fn set_null_with_type_name(
        &mut self,
        index: ParameterIndex,
        sql_type: i32,
        type_name: Option<&str>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Null {
                sql_type,
                type_name: PreparedTypeNameArgument::Specified(type_name.map(str::to_owned)),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNull (String parameterName, int sqlType, String typeName) throws SQLException;
    fn set_null_with_type_name_by_name(
        &mut self,
        name: Option<&str>,
        sql_type: i32,
        type_name: Option<&str>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Null {
                sql_type,
                type_name: PreparedTypeNameArgument::Specified(type_name.map(str::to_owned)),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBoolean(int parameterIndex, boolean x) throws SQLException;
    fn set_boolean(&mut self, index: ParameterIndex, value: bool) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Boolean(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBoolean(String parameterName, boolean x) throws SQLException;
    fn set_boolean_by_name(&mut self, name: Option<&str>, value: bool) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Boolean(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setByte(int parameterIndex, byte x) throws SQLException;
    fn set_byte(&mut self, index: ParameterIndex, value: i8) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Byte(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setByte(String parameterName, byte x) throws SQLException;
    fn set_byte_by_name(&mut self, name: Option<&str>, value: i8) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Byte(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setShort(int parameterIndex, short x) throws SQLException;
    fn set_short(&mut self, index: ParameterIndex, value: i16) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Short(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setShort(String parameterName, short x) throws SQLException;
    fn set_short_by_name(&mut self, name: Option<&str>, value: i16) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Short(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setInt(int parameterIndex, int x) throws SQLException;
    fn set_int(&mut self, index: ParameterIndex, value: i32) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Int(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setInt(String parameterName, int x) throws SQLException;
    fn set_int_by_name(&mut self, name: Option<&str>, value: i32) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Int(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setLong(int parameterIndex, long x) throws SQLException;
    fn set_long(&mut self, index: ParameterIndex, value: i64) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Long(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setLong(String parameterName, long x) throws SQLException;
    fn set_long_by_name(&mut self, name: Option<&str>, value: i64) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Long(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setFloat(int parameterIndex, float x) throws SQLException;
    fn set_float(&mut self, index: ParameterIndex, value: f32) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Float(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setFloat(String parameterName, float x) throws SQLException;
    fn set_float_by_name(&mut self, name: Option<&str>, value: f32) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Float(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setDouble(int parameterIndex, double x) throws SQLException;
    fn set_double(&mut self, index: ParameterIndex, value: f64) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Double(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setDouble(String parameterName, double x) throws SQLException;
    fn set_double_by_name(&mut self, name: Option<&str>, value: f64) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Double(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBigDecimal(int parameterIndex, BigDecimal x) throws SQLException;
    fn set_big_decimal(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcBigDecimal>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::BigDecimal(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBigDecimal(String parameterName, BigDecimal x) throws SQLException;
    fn set_big_decimal_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcBigDecimal>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::BigDecimal(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setString(int parameterIndex, String x) throws SQLException;
    fn set_string(&mut self, index: ParameterIndex, value: Option<&str>) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::String(value.map(str::to_owned)),
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setString(String parameterName, String x) throws SQLException;
    fn set_string_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<&str>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::String(value.map(str::to_owned)),
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBytes(int parameterIndex, byte x[]) throws SQLException;
    fn set_bytes(
        &mut self,
        index: ParameterIndex,
        value: Option<Vec<u8>>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Bytes(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBytes(String parameterName, byte x[]) throws SQLException;
    fn set_bytes_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<Vec<u8>>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Bytes(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setDate(int parameterIndex, java.sql.Date x) throws SQLException;
    fn set_date(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcNaiveDate>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Date {
                value,
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setTime(int parameterIndex, java.sql.Time x) throws SQLException;
    fn set_time(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcNaiveTime>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Time {
                value,
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setTimestamp(int parameterIndex, java.sql.Timestamp x) throws SQLException;
    fn set_timestamp(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcNaiveDateTime>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Timestamp {
                value,
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setTimestamp(String parameterName, java.sql.Timestamp x) throws SQLException;
    fn set_timestamp_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcNaiveDateTime>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Timestamp {
                value,
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setAsciiStream(int parameterIndex, java.io.InputStream x, int length) throws SQLException;
    fn set_ascii_stream_with_length(
        &mut self,
        index: ParameterIndex,
        stream: Option<RdbcInputStream>,
        length: i32,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::AsciiStream {
                stream,
                length: RdbcStreamLength::Int(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setAsciiStream(String parameterName, java.io.InputStream x, int length) throws SQLException;
    fn set_ascii_stream_with_length_by_name(
        &mut self,
        name: Option<&str>,
        stream: Option<RdbcInputStream>,
        length: i32,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::AsciiStream {
                stream,
                length: RdbcStreamLength::Int(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBinaryStream(int parameterIndex, java.io.InputStream x, int length) throws SQLException;
    fn set_binary_stream_with_length(
        &mut self,
        index: ParameterIndex,
        stream: Option<RdbcInputStream>,
        length: i32,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::BinaryStream {
                stream,
                length: RdbcStreamLength::Int(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBinaryStream(String parameterName, java.io.InputStream x, int length) throws SQLException;
    fn set_binary_stream_with_length_by_name(
        &mut self,
        name: Option<&str>,
        stream: Option<RdbcInputStream>,
        length: i32,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::BinaryStream {
                stream,
                length: RdbcStreamLength::Int(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setCharacterStream(int parameterIndex, Reader reader, int length) throws SQLException;
    fn set_character_stream_with_length(
        &mut self,
        index: ParameterIndex,
        reader: Option<RdbcReader>,
        length: i32,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::CharacterStream {
                reader,
                length: RdbcCharacterLength::Int(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setCharacterStream(String parameterName, java.io.Reader reader, int length) throws SQLException;
    fn set_character_stream_with_length_by_name(
        &mut self,
        name: Option<&str>,
        reader: Option<RdbcReader>,
        length: i32,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::CharacterStream {
                reader,
                length: RdbcCharacterLength::Int(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setAsciiStream(int parameterIndex, java.io.InputStream x) throws SQLException;
    fn set_ascii_stream(
        &mut self,
        index: ParameterIndex,
        stream: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::AsciiStream {
                stream,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setAsciiStream(String parameterName, java.io.InputStream x) throws SQLException;
    fn set_ascii_stream_by_name(
        &mut self,
        name: Option<&str>,
        stream: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::AsciiStream {
                stream,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBinaryStream(int parameterIndex, java.io.InputStream x) throws SQLException;
    fn set_binary_stream(
        &mut self,
        index: ParameterIndex,
        stream: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::BinaryStream {
                stream,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBinaryStream(String parameterName, java.io.InputStream x) throws SQLException;
    fn set_binary_stream_by_name(
        &mut self,
        name: Option<&str>,
        stream: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::BinaryStream {
                stream,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setCharacterStream(int parameterIndex, java.io.Reader reader) throws SQLException;
    fn set_character_stream(
        &mut self,
        index: ParameterIndex,
        reader: Option<RdbcReader>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::CharacterStream {
                reader,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setCharacterStream(String parameterName, java.io.Reader reader) throws SQLException;
    fn set_character_stream_by_name(
        &mut self,
        name: Option<&str>,
        reader: Option<RdbcReader>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::CharacterStream {
                reader,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNCharacterStream(int parameterIndex, Reader value) throws SQLException;
    fn set_n_character_stream(
        &mut self,
        index: ParameterIndex,
        reader: Option<RdbcReader>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::NCharacterStream {
                reader,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setObject(int parameterIndex, Object x, int targetSqlType, int scaleOrLength) throws SQLException;
    fn set_object_with_type_and_scale_or_length(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcObject>,
        target_sql_type: i32,
        scale_or_length: i32,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Object {
                value,
                target_sql_type: Some(target_sql_type),
                scale_or_length: Some(scale_or_length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setObject(String parameterName, Object x, int targetSqlType, int scale) throws SQLException;
    fn set_object_with_type_and_scale_or_length_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcObject>,
        target_sql_type: i32,
        scale_or_length: i32,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Object {
                value,
                target_sql_type: Some(target_sql_type),
                scale_or_length: Some(scale_or_length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setObject(int parameterIndex, Object x, int targetSqlType) throws SQLException;
    fn set_object_with_type(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcObject>,
        target_sql_type: i32,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Object {
                value,
                target_sql_type: Some(target_sql_type),
                scale_or_length: None,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setObject(String parameterName, Object x, int targetSqlType) throws SQLException;
    fn set_object_with_type_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcObject>,
        target_sql_type: i32,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Object {
                value,
                target_sql_type: Some(target_sql_type),
                scale_or_length: None,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setObject(String parameterName, Object x) throws SQLException;
    fn set_object_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcObject>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Object {
                value,
                target_sql_type: None,
                scale_or_length: None,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setObject(int parameterIndex, Object x) throws SQLException;
    fn set_object(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcObject>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Object {
                value,
                target_sql_type: None,
                scale_or_length: None,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setRef (int i, Ref x) throws SQLException;
    fn set_ref(&mut self, index: ParameterIndex, value: Option<RdbcRef>) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Ref(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBlob (int i, Blob x) throws SQLException;
    fn set_blob(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcBlob>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Blob(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBlob(int parameterIndex, InputStream inputStream, long length) throws SQLException;
    fn set_blob_stream_with_length(
        &mut self,
        index: ParameterIndex,
        stream: Option<RdbcInputStream>,
        length: i64,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::BlobStream {
                stream,
                length: RdbcStreamLength::Long(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBlob(int parameterIndex, InputStream inputStream) throws SQLException;
    fn set_blob_stream(
        &mut self,
        index: ParameterIndex,
        stream: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::BlobStream {
                stream,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBlob(String parameterName, InputStream inputStream, long length) throws SQLException;
    fn set_blob_stream_with_length_by_name(
        &mut self,
        name: Option<&str>,
        stream: Option<RdbcInputStream>,
        length: i64,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::BlobStream {
                stream,
                length: RdbcStreamLength::Long(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBlob (String parameterName, Blob x) throws SQLException;
    fn set_blob_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcBlob>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Blob(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setBlob(String parameterName, InputStream inputStream) throws SQLException;
    fn set_blob_stream_by_name(
        &mut self,
        name: Option<&str>,
        stream: Option<RdbcInputStream>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::BlobStream {
                stream,
                length: RdbcStreamLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setClob (int i, Clob x) throws SQLException;
    fn set_clob(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcClob>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Clob(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setClob(int parameterIndex, Reader reader, long length) throws SQLException;
    fn set_clob_reader_with_length(
        &mut self,
        index: ParameterIndex,
        reader: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::ClobReader {
                reader,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setClob(int parameterIndex, Reader reader) throws SQLException;
    fn set_clob_reader(
        &mut self,
        index: ParameterIndex,
        reader: Option<RdbcReader>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::ClobReader {
                reader,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setClob(String parameterName, Reader reader, long length) throws SQLException;
    fn set_clob_reader_with_length_by_name(
        &mut self,
        name: Option<&str>,
        reader: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::ClobReader {
                reader,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setClob (String parameterName, Clob x) throws SQLException;
    fn set_clob_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcClob>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::Clob(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setClob(String parameterName, Reader reader) throws SQLException;
    fn set_clob_reader_by_name(
        &mut self,
        name: Option<&str>,
        reader: Option<RdbcReader>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::ClobReader {
                reader,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setArray (int i, Array x) throws SQLException;
    fn set_array(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcArray>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Array(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setDate(int parameterIndex, java.sql.Date x, Calendar cal) throws SQLException;
    fn set_date_with_calendar(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcNaiveDate>,
        calendar: Option<RdbcCalendar>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Date {
                value,
                calendar: RdbcCalendarArgument::Specified(calendar),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setDate(String parameterName, java.sql.Date x) throws SQLException;
    fn set_date_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcNaiveDate>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Date {
                value,
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setDate(String parameterName, java.sql.Date x, Calendar cal) throws SQLException;
    fn set_date_with_calendar_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcNaiveDate>,
        calendar: Option<RdbcCalendar>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Date {
                value,
                calendar: RdbcCalendarArgument::Specified(calendar),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setTime(int parameterIndex, java.sql.Time x, Calendar cal) throws SQLException;
    fn set_time_with_calendar(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcNaiveTime>,
        calendar: Option<RdbcCalendar>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Time {
                value,
                calendar: RdbcCalendarArgument::Specified(calendar),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setTime(String parameterName, java.sql.Time x) throws SQLException;
    fn set_time_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcNaiveTime>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Time {
                value,
                calendar: RdbcCalendarArgument::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setTime(String parameterName, java.sql.Time x, Calendar cal) throws SQLException;
    fn set_time_with_calendar_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcNaiveTime>,
        calendar: Option<RdbcCalendar>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Time {
                value,
                calendar: RdbcCalendarArgument::Specified(calendar),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setTimestamp(int parameterIndex, java.sql.Timestamp x, Calendar cal) throws SQLException;
    fn set_timestamp_with_calendar(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcNaiveDateTime>,
        calendar: Option<RdbcCalendar>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::Timestamp {
                value,
                calendar: RdbcCalendarArgument::Specified(calendar),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setTimestamp(String parameterName, java.sql.Timestamp x, Calendar cal) throws SQLException;
    fn set_timestamp_with_calendar_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcNaiveDateTime>,
        calendar: Option<RdbcCalendar>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::Timestamp {
                value,
                calendar: RdbcCalendarArgument::Specified(calendar),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setSQLXML(int parameterIndex, SQLXML xmlObject) throws SQLException;
    fn set_sql_xml(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcSqlXml>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::SqlXml(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setSQLXML(String parameterName, SQLXML xmlObject) throws SQLException;
    fn set_sql_xml_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcSqlXml>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::SqlXml(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setRowId(int parameterIndex, RowId x) throws SQLException;
    fn set_row_id(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcRowId>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::RowId(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setRowId(String parameterName, RowId x) throws SQLException;
    fn set_row_id_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcRowId>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::RowId(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNString(int parameterIndex, String value) throws SQLException;
    fn set_n_string(
        &mut self,
        index: ParameterIndex,
        value: Option<&str>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::NString(value.map(str::to_owned)),
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#public void setNString(String parameterName, String value) throws SQLException;
    fn set_n_string_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<&str>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::NString(value.map(str::to_owned)),
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNCharacterStream(int parameterIndex, Reader value, long length) throws SQLException;
    fn set_n_character_stream_with_length(
        &mut self,
        index: ParameterIndex,
        reader: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::NCharacterStream {
                reader,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#public void setNCharacterStream(String parameterName, Reader value, long length) throws SQLException;
    fn set_n_character_stream_with_length_by_name(
        &mut self,
        name: Option<&str>,
        reader: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::NCharacterStream {
                reader,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNCharacterStream(String parameterName, Reader value) throws SQLException;
    fn set_n_character_stream_by_name(
        &mut self,
        name: Option<&str>,
        reader: Option<RdbcReader>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::NCharacterStream {
                reader,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNClob(String parameterName, NClob value) throws SQLException;
    fn set_n_clob_by_name(
        &mut self,
        name: Option<&str>,
        value: Option<RdbcNClob>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(name, PreparedInputParameter::NClob(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNClob(String parameterName, Reader reader, long length) throws SQLException;
    fn set_n_clob_reader_with_length_by_name(
        &mut self,
        name: Option<&str>,
        reader: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::NClobReader {
                reader,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNClob(String parameterName, Reader reader) throws SQLException;
    fn set_n_clob_reader_by_name(
        &mut self,
        name: Option<&str>,
        reader: Option<RdbcReader>,
    ) -> Result<(), RdbcError> {
        self.set_named_parameter(
            name,
            PreparedInputParameter::NClobReader {
                reader,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNClob(int parameterIndex, Reader reader, long length) throws SQLException;
    fn set_n_clob_reader_with_length(
        &mut self,
        index: ParameterIndex,
        reader: Option<RdbcReader>,
        length: i64,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::NClobReader {
                reader,
                length: RdbcCharacterLength::Long(length),
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNClob(int parameterIndex, NClob value) throws SQLException;
    fn set_n_clob(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcNClob>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::NClob(value))
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setNClob(int parameterIndex, Reader reader) throws SQLException;
    fn set_n_clob_reader(
        &mut self,
        index: ParameterIndex,
        reader: Option<RdbcReader>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(
            index,
            PreparedInputParameter::NClobReader {
                reader,
                length: RdbcCharacterLength::Unspecified,
            },
        )
    }
    /// 按原 selector 和参数保存对应重载，返回绑定结果，不提前读取或关闭资源。
    /// 对应 Java: javax.sql.RowSet#void setURL(int parameterIndex, java.net.URL x) throws SQLException;
    fn set_parameter_url(
        &mut self,
        index: ParameterIndex,
        value: Option<RdbcUrl>,
    ) -> Result<(), RdbcError> {
        self.set_parameter(index, PreparedInputParameter::Url(value))
    }
}
