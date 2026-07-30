//! 对外池化 CallableStatement。
//!
//! 对应 Java：
//! `com.alibaba.druid.pool.DruidPooledCallableStatement`。
//! 来源文件：
//! `core/src/main/java/com/alibaba/druid/pool/DruidPooledCallableStatement.java`。

use super::{
    CallableCalendar, CallableCalendarArgument, CallableInputParameter, CallableOutParameter,
    CallableParameter, DruidError, DruidPooledConnection, DruidPooledPreparedStatement,
    DruidPooledPreparedStatementHandle, DruidPooledResultSet, DruidPooledStatement, ExecResult,
    JdbcArray, JdbcBlob, JdbcCharacterLength, JdbcClob, JdbcInputStream, JdbcNClob, JdbcObject,
    JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcStreamLength, JdbcTargetType, JdbcTypeMap,
    JdbcUrl, PhysicalCallableStatement, PhysicalPreparedStatement, PreparedStatementKey, Row,
    Unwrapped, Value, Wrapper,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::any::{Any, TypeId};

/// 池化存储过程调用语句。
///
/// 组合 `DruidPooledPreparedStatement` 复用 Java 的 prepared 缓存、异常删除、
/// fetch peak、逻辑关闭和连接租约约束；CallableStatement 特有行为委托给
/// `PhysicalCallableStatement`。
pub struct DruidPooledCallableStatement {
    prepared_statement: DruidPooledPreparedStatement,
}

/// `ResultSet#getStatement()` 返回的 CallableStatement 共享身份句柄。
///
/// 对应 Java：`DruidPooledCallableStatement` 继承
/// `DruidPooledPreparedStatement`，结果集保存并返回原 callable 对象。Rust
/// 句柄保留相同的 prepared 生命周期、物理 callable 能力和关闭级联。
#[derive(Clone)]
pub struct DruidPooledCallableStatementHandle {
    prepared_statement: DruidPooledPreparedStatementHandle,
}

impl DruidPooledCallableStatementHandle {
    /// 返回继承自 Statement proxy 的 ID。
    #[must_use]
    pub fn id(&self) -> u64 {
        self.prepared_statement.id()
    }

    /// 返回完整缓存键。
    pub fn key(&self) -> &PreparedStatementKey {
        self.prepared_statement.key()
    }

    /// 返回继承的池化 Statement 视图。
    pub fn pooled_statement(&self) -> &DruidPooledStatement {
        self.prepared_statement.pooled_statement()
    }

    /// 返回继承的 PreparedStatement 身份视图。
    pub fn prepared_statement(&self) -> &DruidPooledPreparedStatementHandle {
        &self.prepared_statement
    }

    /// 返回原逻辑 CallableStatement 是否已关闭。
    pub fn is_closed(&self) -> bool {
        self.prepared_statement.is_closed()
    }

    /// 判断句柄是否与给定 CallableStatement 表示同一逻辑 Java 对象。
    pub fn is_same_statement(&self, statement: &DruidPooledCallableStatement) -> bool {
        self.prepared_statement
            .is_same_statement(&statement.prepared_statement)
    }

    /// 关闭原逻辑 CallableStatement。
    pub fn close(&self) -> Result<(), DruidError> {
        self.prepared_statement.close()
    }
}

impl Wrapper for DruidPooledCallableStatementHandle {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        let Some(iface) = iface else {
            return false;
        };
        iface == TypeId::of::<Self>()
            || iface == TypeId::of::<dyn PhysicalCallableStatement>()
            || self.prepared_statement.is_wrapper_for(Some(iface))
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if iface == TypeId::of::<Self>() {
            return Some(Unwrapped::Object(self));
        }
        if iface == TypeId::of::<dyn PhysicalCallableStatement>() {
            return self
                .prepared_statement
                .physical_statement()
                .as_callable()
                .map(Unwrapped::CallableStatement);
        }
        self.prepared_statement.unwrap(Some(iface))
    }
}

impl Wrapper for DruidPooledCallableStatement {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        let Some(iface) = iface else {
            return false;
        };
        let raw = self
            .prepared_statement
            .prepared_statement_holder()
            .statement();
        iface == TypeId::of::<Self>()
            || iface == TypeId::of::<dyn PhysicalCallableStatement>()
            || iface == TypeId::of::<dyn PhysicalPreparedStatement>()
            || raw.as_any().type_id() == iface
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if iface == TypeId::of::<Self>() {
            return Some(Unwrapped::Object(self));
        }
        let raw = self
            .prepared_statement
            .prepared_statement_holder()
            .statement();
        if iface == TypeId::of::<dyn PhysicalCallableStatement>() {
            return raw.as_callable().map(Unwrapped::CallableStatement);
        }
        if iface == TypeId::of::<dyn PhysicalPreparedStatement>() {
            return Some(Unwrapped::PreparedStatement(raw.as_ref()));
        }
        (raw.as_any().type_id() == iface).then(|| Unwrapped::Object(raw.as_any()))
    }
}

impl std::fmt::Debug for DruidPooledCallableStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledCallableStatement")
            .field("prepared_statement", &self.prepared_statement)
            .finish()
    }
}

impl DruidPooledCallableStatement {
    /// 返回继承自 Statement proxy 的 ID。
    #[must_use]
    pub fn id(&self) -> u64 {
        self.prepared_statement.id()
    }

    pub(crate) fn new(prepared_statement: DruidPooledPreparedStatement) -> Self {
        Self { prepared_statement }
    }

    /// 返回完整缓存键。
    pub fn key(&self) -> &PreparedStatementKey {
        self.prepared_statement.key()
    }

    /// 返回逻辑语句是否关闭。
    pub fn is_closed(&self) -> bool {
        self.prepared_statement.is_closed()
    }

    /// 返回物理 CallableStatement SPI。
    ///
    /// 对应 Java：`getCallableStatementRaw()` 的平台能力语义，但不泄漏具体驱动类型。
    pub fn physical_callable_statement(
        &self,
    ) -> Result<&dyn PhysicalCallableStatement, DruidError> {
        self.prepared_statement.ensure_open()?;
        self.prepared_statement
            .prepared_statement_holder()
            .statement()
            .as_callable()
            .ok_or(DruidError::UnsupportedOperation {
                operation: "physical_callable_statement",
            })
    }

    /// 执行存储过程调用。
    pub async fn exec(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        self.prepared_statement.exec(connection, params).await
    }

    /// 执行返回行集的存储过程调用。
    pub async fn fetch(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        self.prepared_statement.fetch(connection, params).await
    }

    /// 执行查询并返回保持 CallableStatement 动态身份的池化结果集。
    pub async fn fetch_result_set(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<DruidPooledResultSet, DruidError> {
        let result_set = self
            .prepared_statement
            .fetch_result_set(connection, params)
            .await?;
        Ok(result_set.with_callable_statement(self.result_set_statement_handle()))
    }

    /// 执行 `CallableStatement#execute()` 并返回首结果是否为 ResultSet。
    pub async fn execute(
        &mut self,
        connection: &mut DruidPooledConnection,
        params: Vec<Value>,
    ) -> Result<bool, DruidError> {
        self.prepared_statement.execute(connection, params).await
    }

    /// 返回 generic execute 的当前结果集，并恢复 CallableStatement 身份。
    pub fn result_set(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<Option<DruidPooledResultSet>, DruidError> {
        self.prepared_statement
            .result_set(connection)
            .map(|result_set| {
                result_set.map(|result_set| {
                    result_set.with_callable_statement(self.result_set_statement_handle())
                })
            })
    }

    /// 返回最近一次执行的更新计数。
    pub fn update_count(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<i64, DruidError> {
        self.prepared_statement.update_count(connection)
    }

    /// 返回 generated keys，并恢复 CallableStatement 身份。
    pub fn generated_keys(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<DruidPooledResultSet, DruidError> {
        self.prepared_statement
            .generated_keys(connection)
            .map(|result_set| {
                result_set.with_callable_statement(self.result_set_statement_handle())
            })
    }

    /// 推进到下一个 JDBC 结果。
    pub fn more_results(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<bool, DruidError> {
        self.prepared_statement.more_results(connection)
    }

    /// 使用 JDBC current 常量推进到下一个结果。
    pub fn more_results_with_current(
        &mut self,
        connection: &mut DruidPooledConnection,
        current: i32,
    ) -> Result<bool, DruidError> {
        self.prepared_statement
            .more_results_with_current(connection, current)
    }

    /// 关闭逻辑 CallableStatement。
    pub fn close(&mut self) -> Result<(), DruidError> {
        self.prepared_statement.close()
    }

    /// 在原池化连接上下文中关闭 CallableStatement。
    ///
    /// 对应 Java：`DruidPooledConnection#closePoolableStatement`。除执行
    /// PreparedStatement 的缓存归还与异常分类外，还会从 holder 的
    /// statement trace 中移除该逻辑对象。
    pub fn close_with_connection(
        &mut self,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        self.prepared_statement.close_with_connection(connection)
    }

    fn result_set_statement_handle(&self) -> DruidPooledCallableStatementHandle {
        DruidPooledCallableStatementHandle {
            prepared_statement: self.prepared_statement.result_set_statement_handle(),
        }
    }

    /// 注册 `registerOutParameter(int, int)`。
    pub fn register_out_parameter(
        &mut self,
        parameter_index: usize,
        sql_type: i32,
    ) -> Result<(), DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.register(parameter, CallableOutParameter::new(sql_type))
    }

    /// 注册 `registerOutParameter(int, int, int)`。
    pub fn register_out_parameter_with_scale(
        &mut self,
        parameter_index: usize,
        sql_type: i32,
        scale: i32,
    ) -> Result<(), DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.register(parameter, CallableOutParameter::with_scale(sql_type, scale))
    }

    /// 注册 `registerOutParameter(int, int, String)`。
    pub fn register_out_parameter_with_type_name(
        &mut self,
        parameter_index: usize,
        sql_type: i32,
        type_name: impl Into<String>,
    ) -> Result<(), DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.register(
            parameter,
            CallableOutParameter::with_type_name(sql_type, type_name),
        )
    }

    /// 注册 `registerOutParameter(String, int)`。
    pub fn register_named_out_parameter(
        &mut self,
        parameter_name: impl Into<String>,
        sql_type: i32,
    ) -> Result<(), DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.register(parameter, CallableOutParameter::new(sql_type))
    }

    /// 注册 `registerOutParameter(String, int, int)`。
    pub fn register_named_out_parameter_with_scale(
        &mut self,
        parameter_name: impl Into<String>,
        sql_type: i32,
        scale: i32,
    ) -> Result<(), DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.register(parameter, CallableOutParameter::with_scale(sql_type, scale))
    }

    /// 注册 `registerOutParameter(String, int, String)`。
    pub fn register_named_out_parameter_with_type_name(
        &mut self,
        parameter_name: impl Into<String>,
        sql_type: i32,
        type_name: impl Into<String>,
    ) -> Result<(), DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.register(
            parameter,
            CallableOutParameter::with_type_name(sql_type, type_name),
        )
    }

    /// 设置命名参数的通用值。
    pub fn set_named_object(
        &mut self,
        parameter_name: &str,
        value: Value,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::object(value))
    }

    /// 设置带目标 SQL 类型的命名对象参数。
    ///
    /// 对应 Java：`setObject(String parameterName, Object x, int targetSqlType)`。
    pub fn set_named_object_with_sql_type(
        &mut self,
        parameter_name: &str,
        value: Value,
        target_sql_type: i32,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::object_with_sql_type(value, target_sql_type),
        )
    }

    /// 设置带目标 SQL 类型和 scale 的命名对象参数。
    ///
    /// 对应 Java：
    /// `setObject(String parameterName, Object x, int targetSqlType, int scale)`。
    pub fn set_named_object_with_sql_type_and_scale(
        &mut self,
        parameter_name: &str,
        value: Value,
        target_sql_type: i32,
        scale: i32,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::object_with_sql_type_and_scale(value, target_sql_type, scale),
        )
    }

    /// 设置命名 SQL NULL。
    ///
    /// 对应 Java：`setNull(String parameterName, int sqlType)`。
    pub fn set_named_null(
        &mut self,
        parameter_name: &str,
        sql_type: i32,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::null(sql_type))
    }

    /// 设置带数据库类型名的命名 SQL NULL。
    ///
    /// 对应 Java：
    /// `setNull(String parameterName, int sqlType, String typeName)`。
    pub fn set_named_null_with_type_name(
        &mut self,
        parameter_name: &str,
        sql_type: i32,
        type_name: impl Into<String>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::null_with_type_name(sql_type, type_name),
        )
    }

    fn set_named_input(
        &mut self,
        parameter_name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), DruidError> {
        let parameter_name = match self.named_parameter(parameter_name)? {
            CallableParameter::Name(parameter_name) => parameter_name,
            CallableParameter::Index(_) => {
                unreachable!("named parameter validation returned index")
            }
        };
        self.apply_callable(|statement| statement.set_named_parameter(&parameter_name, parameter))
    }

    /// 设置命名布尔参数。
    pub fn set_named_boolean(
        &mut self,
        parameter_name: &str,
        value: bool,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Boolean(value))
    }

    /// 设置命名 byte 参数。
    ///
    /// 对应 Java：`setByte(String parameterName, byte x)`。
    pub fn set_named_byte(&mut self, parameter_name: &str, value: i8) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Byte(value))
    }

    /// 设置命名 short 参数。
    ///
    /// 对应 Java：`setShort(String parameterName, short x)`。
    pub fn set_named_short(&mut self, parameter_name: &str, value: i16) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Short(value))
    }

    /// 设置命名整数参数。
    pub fn set_named_int(&mut self, parameter_name: &str, value: i32) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Int(value))
    }

    /// 设置命名长整数参数。
    pub fn set_named_long(&mut self, parameter_name: &str, value: i64) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Long(value))
    }

    /// 设置命名单精度参数。
    ///
    /// 对应 Java：`setFloat(String parameterName, float x)`。
    pub fn set_named_float(&mut self, parameter_name: &str, value: f32) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Float(value))
    }

    /// 设置命名浮点参数。
    pub fn set_named_double(&mut self, parameter_name: &str, value: f64) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Double(value))
    }

    /// 设置命名字符串参数。
    pub fn set_named_string(
        &mut self,
        parameter_name: &str,
        value: Option<String>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::String(value))
    }

    /// 设置命名 Unicode NString 参数。
    ///
    /// 对应 Java：`setNString(String parameterName, String value)`。独立 variant
    /// 保留 national-character setter 身份，交由具体驱动映射。
    pub fn set_named_n_string(
        &mut self,
        parameter_name: &str,
        value: Option<String>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::NString(value))
    }

    /// 设置命名字节参数。
    pub fn set_named_bytes(
        &mut self,
        parameter_name: &str,
        value: Option<Vec<u8>>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Bytes(value))
    }

    /// 设置命名 URL 参数。
    pub fn set_named_url(
        &mut self,
        parameter_name: &str,
        value: Option<JdbcUrl>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Url(value))
    }

    /// 设置命名 `RowId` 参数。
    pub fn set_named_row_id(
        &mut self,
        parameter_name: &str,
        value: Option<JdbcRowId>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::RowId(value))
    }

    /// 设置命名 `SQLXML` 参数。
    pub fn set_named_sql_xml(
        &mut self,
        parameter_name: &str,
        value: Option<JdbcSqlXml>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::SqlXml(value))
    }

    /// 设置未指定长度的命名 ASCII 输入流。
    pub fn set_named_ascii_stream(
        &mut self,
        parameter_name: &str,
        stream: Option<JdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::AsciiStream {
                stream,
                length: JdbcStreamLength::Unspecified,
            },
        )
    }

    /// 设置带 Java int 长度的命名 ASCII 输入流。
    pub fn set_named_ascii_stream_with_int_length(
        &mut self,
        parameter_name: &str,
        stream: Option<JdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::AsciiStream {
                stream,
                length: JdbcStreamLength::Int(length),
            },
        )
    }

    /// 设置带 Java long 长度的命名 ASCII 输入流。
    pub fn set_named_ascii_stream_with_length(
        &mut self,
        parameter_name: &str,
        stream: Option<JdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::AsciiStream {
                stream,
                length: JdbcStreamLength::Long(length),
            },
        )
    }

    /// 设置未指定长度的命名二进制输入流。
    pub fn set_named_binary_stream(
        &mut self,
        parameter_name: &str,
        stream: Option<JdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::BinaryStream {
                stream,
                length: JdbcStreamLength::Unspecified,
            },
        )
    }

    /// 设置带 Java int 长度的命名二进制输入流。
    pub fn set_named_binary_stream_with_int_length(
        &mut self,
        parameter_name: &str,
        stream: Option<JdbcInputStream>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::BinaryStream {
                stream,
                length: JdbcStreamLength::Int(length),
            },
        )
    }

    /// 设置带 Java long 长度的命名二进制输入流。
    pub fn set_named_binary_stream_with_length(
        &mut self,
        parameter_name: &str,
        stream: Option<JdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::BinaryStream {
                stream,
                length: JdbcStreamLength::Long(length),
            },
        )
    }

    /// 设置命名 Blob 对象参数。
    ///
    /// 对应 Java：`setBlob(String parameterName, Blob x)`。`None` 保留通过
    /// Blob setter 传入 Java null 的身份，不等价于 `setNull`。
    pub fn set_named_blob(
        &mut self,
        parameter_name: &str,
        value: Option<JdbcBlob>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Blob(value))
    }

    /// 设置未指定长度的命名 Blob 输入流。
    ///
    /// 对应 Java：`setBlob(String parameterName, InputStream inputStream)`。
    pub fn set_named_blob_stream(
        &mut self,
        parameter_name: &str,
        stream: Option<JdbcInputStream>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::BlobStream {
                stream,
                length: JdbcStreamLength::Unspecified,
            },
        )
    }

    /// 设置带 long 长度的命名 Blob 输入流。
    ///
    /// 对应 Java：
    /// `setBlob(String parameterName, InputStream inputStream, long length)`。
    /// 长度原样传给物理驱动，包括驱动应拒绝的负值。
    pub fn set_named_blob_stream_with_length(
        &mut self,
        parameter_name: &str,
        stream: Option<JdbcInputStream>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::BlobStream {
                stream,
                length: JdbcStreamLength::Long(length),
            },
        )
    }

    /// 设置命名 Clob 对象参数。
    ///
    /// 对应 Java：`setClob(String parameterName, Clob x)`。
    pub fn set_named_clob(
        &mut self,
        parameter_name: &str,
        value: Option<JdbcClob>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::Clob(value))
    }

    /// 设置未指定长度的命名 Clob Reader。
    ///
    /// 对应 Java：`setClob(String parameterName, Reader reader)`。
    pub fn set_named_clob_reader(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::ClobReader {
                reader,
                length: JdbcCharacterLength::Unspecified,
            },
        )
    }

    /// 设置带 long 长度的命名 Clob Reader。
    ///
    /// 对应 Java：`setClob(String parameterName, Reader reader, long length)`。
    pub fn set_named_clob_reader_with_length(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::ClobReader {
                reader,
                length: JdbcCharacterLength::Long(length),
            },
        )
    }

    /// 设置命名 NClob 对象参数。
    ///
    /// 对应 Java：`setNClob(String parameterName, NClob value)`。
    pub fn set_named_n_clob(
        &mut self,
        parameter_name: &str,
        value: Option<JdbcNClob>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::NClob(value))
    }

    /// 设置未指定长度的命名 NClob Reader。
    ///
    /// 对应 Java：`setNClob(String parameterName, Reader reader)`。
    pub fn set_named_n_clob_reader(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::NClobReader {
                reader,
                length: JdbcCharacterLength::Unspecified,
            },
        )
    }

    /// 设置带 long 长度的命名 NClob Reader。
    ///
    /// 对应 Java：`setNClob(String parameterName, Reader reader, long length)`。
    pub fn set_named_n_clob_reader_with_length(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::NClobReader {
                reader,
                length: JdbcCharacterLength::Long(length),
            },
        )
    }

    /// 设置未指定长度的普通字符 Reader。
    ///
    /// 对应 Java：`setCharacterStream(String parameterName, Reader reader)`。
    pub fn set_named_character_stream(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::CharacterStream {
                reader,
                length: JdbcCharacterLength::Unspecified,
            },
        )
    }

    /// 设置带 int 长度的普通字符 Reader。
    ///
    /// 对应 Java：`setCharacterStream(String parameterName, Reader reader, int length)`。
    pub fn set_named_character_stream_with_int_length(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
        length: i32,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::CharacterStream {
                reader,
                length: JdbcCharacterLength::Int(length),
            },
        )
    }

    /// 设置带 long 长度的普通字符 Reader。
    ///
    /// 对应 Java：`setCharacterStream(String parameterName, Reader reader, long length)`。
    pub fn set_named_character_stream_with_length(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::CharacterStream {
                reader,
                length: JdbcCharacterLength::Long(length),
            },
        )
    }

    /// 设置未指定长度的 national character Reader。
    ///
    /// 对应 Java：`setNCharacterStream(String parameterName, Reader value)`。
    pub fn set_named_n_character_stream(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::NCharacterStream {
                reader,
                length: JdbcCharacterLength::Unspecified,
            },
        )
    }

    /// 设置带 long 长度的 national character Reader。
    ///
    /// 对应 Java：`setNCharacterStream(String parameterName, Reader value, long length)`。
    pub fn set_named_n_character_stream_with_length(
        &mut self,
        parameter_name: &str,
        reader: Option<JdbcReader>,
        length: i64,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::NCharacterStream {
                reader,
                length: JdbcCharacterLength::Long(length),
            },
        )
    }

    /// 设置命名任意精度 Decimal 参数。
    ///
    /// 对应 Java：`setBigDecimal(String parameterName, BigDecimal x)`。
    /// `None` 对应通过该 setter 传入 Java null，不等价于 `setNull`。
    pub fn set_named_big_decimal(
        &mut self,
        parameter_name: &str,
        value: Option<BigDecimal>,
    ) -> Result<(), DruidError> {
        self.set_named_input(parameter_name, CallableInputParameter::BigDecimal(value))
    }

    /// 设置不带 Calendar 的命名 Date 参数。
    ///
    /// 对应 Java：`setDate(String parameterName, Date x)`。
    pub fn set_named_date(
        &mut self,
        parameter_name: &str,
        value: Option<NaiveDate>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::Date {
                value,
                calendar: CallableCalendarArgument::unspecified(),
            },
        )
    }

    /// 设置带 Calendar 重载的命名 Date 参数。
    ///
    /// 对应 Java：`setDate(String parameterName, Date x, Calendar cal)`。
    /// `calendar=None` 保留“显式调用 Calendar 重载并传入 null”的身份。
    pub fn set_named_date_with_calendar(
        &mut self,
        parameter_name: &str,
        value: Option<NaiveDate>,
        calendar: Option<CallableCalendar>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::Date {
                value,
                calendar: CallableCalendarArgument::specified(calendar),
            },
        )
    }

    /// 设置不带 Calendar 的命名 Time 参数。
    ///
    /// 对应 Java：`setTime(String parameterName, Time x)`。
    pub fn set_named_time(
        &mut self,
        parameter_name: &str,
        value: Option<NaiveTime>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::Time {
                value,
                calendar: CallableCalendarArgument::unspecified(),
            },
        )
    }

    /// 设置带 Calendar 重载的命名 Time 参数。
    ///
    /// 对应 Java：`setTime(String parameterName, Time x, Calendar cal)`。
    pub fn set_named_time_with_calendar(
        &mut self,
        parameter_name: &str,
        value: Option<NaiveTime>,
        calendar: Option<CallableCalendar>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::Time {
                value,
                calendar: CallableCalendarArgument::specified(calendar),
            },
        )
    }

    /// 设置不带 Calendar 的命名 Timestamp 参数。
    ///
    /// 对应 Java：`setTimestamp(String parameterName, Timestamp x)`。
    pub fn set_named_timestamp(
        &mut self,
        parameter_name: &str,
        value: Option<NaiveDateTime>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::Timestamp {
                value,
                calendar: CallableCalendarArgument::unspecified(),
            },
        )
    }

    /// 设置带 Calendar 重载的命名 Timestamp 参数。
    ///
    /// 对应 Java：
    /// `setTimestamp(String parameterName, Timestamp x, Calendar cal)`。
    pub fn set_named_timestamp_with_calendar(
        &mut self,
        parameter_name: &str,
        value: Option<NaiveDateTime>,
        calendar: Option<CallableCalendar>,
    ) -> Result<(), DruidError> {
        self.set_named_input(
            parameter_name,
            CallableInputParameter::Timestamp {
                value,
                calendar: CallableCalendarArgument::specified(calendar),
            },
        )
    }

    /// 读取索引 OUT 参数。
    ///
    /// 对应 Java：`getObject(int)` 的已迁移标量部分。ResultSet/LOB/Ref/Array
    /// 必须由后续独立对象 SPI 表达，不能伪装成标量。
    pub fn get_object(&mut self, parameter_index: usize) -> Result<JdbcObject, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.get(parameter)
    }

    /// 读取命名 OUT 参数。
    ///
    /// 对应 Java：`getObject(String)` 的已迁移标量部分。
    pub fn get_named_object(&mut self, parameter_name: &str) -> Result<JdbcObject, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.get(parameter)
    }

    /// 使用 Java 类型 Map 读取索引对象。
    pub fn get_object_with_type_map(
        &mut self,
        parameter_index: usize,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        let result = self.apply_callable(|statement| {
            statement.out_parameter_with_type_map(&parameter, type_map)
        });
        self.record_object_lob_result(&result);
        result
    }

    /// 使用 Java 类型 Map 读取命名对象。
    pub fn get_named_object_with_type_map(
        &mut self,
        parameter_name: &str,
        type_map: Option<&JdbcTypeMap>,
    ) -> Result<JdbcObject, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        let result = self.apply_callable(|statement| {
            statement.out_parameter_with_type_map(&parameter, type_map)
        });
        self.record_object_lob_result(&result);
        result
    }

    /// 使用 Java `Class<T>` 对应目标类型读取索引对象。
    pub fn get_object_as(
        &mut self,
        parameter_index: usize,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        // Java 4.1 typed `getObject` 直接委托给底层 stmt，不经过 checkException。
        let parameter = CallableParameter::by_index(parameter_index)?;
        self.physical_callable_statement()?
            .out_parameter_as(&parameter, target_type)
    }

    /// 使用 Java `Class<T>` 对应目标类型读取命名对象。
    pub fn get_named_object_as(
        &mut self,
        parameter_name: &str,
        target_type: &JdbcTargetType,
    ) -> Result<JdbcObject, DruidError> {
        // 与 Java 原方法一致：保留底层异常，不写入连接异常计数。
        let parameter = CallableParameter::by_name(parameter_name)?;
        self.physical_callable_statement()?
            .out_parameter_as(&parameter, target_type)
    }

    /// 读取索引 BigDecimal OUT 参数。
    ///
    /// 对应 Java：`getBigDecimal(int)`。
    pub fn get_big_decimal(
        &mut self,
        parameter_index: usize,
    ) -> Result<Option<BigDecimal>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.big_decimal_out_parameter(&parameter))
    }

    /// 读取索引 BigDecimal OUT 参数并应用已废弃 JDBC scale 重载。
    ///
    /// 对应 Java：`getBigDecimal(int, int)`。
    #[deprecated(note = "对应 JDBC 已废弃的 getBigDecimal(int, int)")]
    pub fn get_big_decimal_with_scale(
        &mut self,
        parameter_index: usize,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| {
            statement.big_decimal_out_parameter_with_scale(&parameter, scale)
        })
    }

    /// 读取命名 BigDecimal OUT 参数。
    ///
    /// 对应 Java：`getBigDecimal(String)`。
    pub fn get_named_big_decimal(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<BigDecimal>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.big_decimal_out_parameter(&parameter))
    }

    /// 读取不带 Calendar 的索引 Date OUT 参数。
    pub fn get_date(&mut self, parameter_index: usize) -> Result<Option<NaiveDate>, DruidError> {
        self.get_date_with_argument(
            CallableParameter::by_index(parameter_index),
            CallableCalendarArgument::unspecified(),
        )
    }

    /// 读取带 Calendar 重载的索引 Date OUT 参数。
    pub fn get_date_with_calendar(
        &mut self,
        parameter_index: usize,
        calendar: Option<CallableCalendar>,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.get_date_with_argument(
            CallableParameter::by_index(parameter_index),
            CallableCalendarArgument::specified(calendar),
        )
    }

    /// 读取不带 Calendar 的命名 Date OUT 参数。
    pub fn get_named_date(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.get_date_with_argument(
            CallableParameter::by_name(parameter_name),
            CallableCalendarArgument::unspecified(),
        )
    }

    /// 读取带 Calendar 重载的命名 Date OUT 参数。
    pub fn get_named_date_with_calendar(
        &mut self,
        parameter_name: &str,
        calendar: Option<CallableCalendar>,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.get_date_with_argument(
            CallableParameter::by_name(parameter_name),
            CallableCalendarArgument::specified(calendar),
        )
    }

    /// 读取不带 Calendar 的索引 Time OUT 参数。
    pub fn get_time(&mut self, parameter_index: usize) -> Result<Option<NaiveTime>, DruidError> {
        self.get_time_with_argument(
            CallableParameter::by_index(parameter_index),
            CallableCalendarArgument::unspecified(),
        )
    }

    /// 读取带 Calendar 重载的索引 Time OUT 参数。
    pub fn get_time_with_calendar(
        &mut self,
        parameter_index: usize,
        calendar: Option<CallableCalendar>,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.get_time_with_argument(
            CallableParameter::by_index(parameter_index),
            CallableCalendarArgument::specified(calendar),
        )
    }

    /// 读取不带 Calendar 的命名 Time OUT 参数。
    pub fn get_named_time(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.get_time_with_argument(
            CallableParameter::by_name(parameter_name),
            CallableCalendarArgument::unspecified(),
        )
    }

    /// 读取带 Calendar 重载的命名 Time OUT 参数。
    pub fn get_named_time_with_calendar(
        &mut self,
        parameter_name: &str,
        calendar: Option<CallableCalendar>,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.get_time_with_argument(
            CallableParameter::by_name(parameter_name),
            CallableCalendarArgument::specified(calendar),
        )
    }

    /// 读取不带 Calendar 的索引 Timestamp OUT 参数。
    pub fn get_timestamp(
        &mut self,
        parameter_index: usize,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.get_timestamp_with_argument(
            CallableParameter::by_index(parameter_index),
            CallableCalendarArgument::unspecified(),
        )
    }

    /// 读取带 Calendar 重载的索引 Timestamp OUT 参数。
    pub fn get_timestamp_with_calendar(
        &mut self,
        parameter_index: usize,
        calendar: Option<CallableCalendar>,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.get_timestamp_with_argument(
            CallableParameter::by_index(parameter_index),
            CallableCalendarArgument::specified(calendar),
        )
    }

    /// 读取不带 Calendar 的命名 Timestamp OUT 参数。
    pub fn get_named_timestamp(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.get_timestamp_with_argument(
            CallableParameter::by_name(parameter_name),
            CallableCalendarArgument::unspecified(),
        )
    }

    /// 读取带 Calendar 重载的命名 Timestamp OUT 参数。
    pub fn get_named_timestamp_with_calendar(
        &mut self,
        parameter_name: &str,
        calendar: Option<CallableCalendar>,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.get_timestamp_with_argument(
            CallableParameter::by_name(parameter_name),
            CallableCalendarArgument::specified(calendar),
        )
    }

    /// 返回最近一次 OUT 参数读取是否得到 SQL NULL。
    pub fn was_null(&mut self) -> Result<bool, DruidError> {
        self.apply_callable(|statement| statement.was_null())
    }

    /// 读取索引字符串 OUT 参数。
    pub fn get_string(&mut self, parameter_index: usize) -> Result<Option<String>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.string_out_parameter(&parameter))
    }

    /// 读取命名字符串 OUT 参数。
    pub fn get_named_string(&mut self, parameter_name: &str) -> Result<Option<String>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.string_out_parameter(&parameter))
    }

    /// 读取索引布尔 OUT 参数；SQL NULL 与 JDBC 一致返回 `false`。
    pub fn get_boolean(&mut self, parameter_index: usize) -> Result<bool, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.boolean_out_parameter(&parameter))
    }

    /// 读取命名布尔 OUT 参数；SQL NULL 与 JDBC 一致返回 `false`。
    pub fn get_named_boolean(&mut self, parameter_name: &str) -> Result<bool, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.boolean_out_parameter(&parameter))
    }

    /// 读取索引 byte OUT 参数；SQL NULL 与 JDBC 一致返回 `0`。
    pub fn get_byte(&mut self, parameter_index: usize) -> Result<i8, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.byte_out_parameter(&parameter))
    }

    /// 读取命名 byte OUT 参数；SQL NULL 与 JDBC 一致返回 `0`。
    pub fn get_named_byte(&mut self, parameter_name: &str) -> Result<i8, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.byte_out_parameter(&parameter))
    }

    /// 读取索引 short OUT 参数；SQL NULL 与 JDBC 一致返回 `0`。
    pub fn get_short(&mut self, parameter_index: usize) -> Result<i16, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.short_out_parameter(&parameter))
    }

    /// 读取命名 short OUT 参数；SQL NULL 与 JDBC 一致返回 `0`。
    pub fn get_named_short(&mut self, parameter_name: &str) -> Result<i16, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.short_out_parameter(&parameter))
    }

    /// 读取索引整数 OUT 参数；SQL NULL 与 JDBC 一致返回 `0`。
    pub fn get_int(&mut self, parameter_index: usize) -> Result<i32, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.int_out_parameter(&parameter))
    }

    /// 读取命名整数 OUT 参数；SQL NULL 与 JDBC 一致返回 `0`。
    pub fn get_named_int(&mut self, parameter_name: &str) -> Result<i32, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.int_out_parameter(&parameter))
    }

    /// 读取索引长整数 OUT 参数。
    pub fn get_long(&mut self, parameter_index: usize) -> Result<i64, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.long_out_parameter(&parameter))
    }

    /// 读取命名长整数 OUT 参数。
    pub fn get_named_long(&mut self, parameter_name: &str) -> Result<i64, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.long_out_parameter(&parameter))
    }

    /// 读取索引 float OUT 参数。
    pub fn get_float(&mut self, parameter_index: usize) -> Result<f32, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.float_out_parameter(&parameter))
    }

    /// 读取命名 float OUT 参数。
    pub fn get_named_float(&mut self, parameter_name: &str) -> Result<f32, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.float_out_parameter(&parameter))
    }

    /// 读取索引双精度 OUT 参数。
    pub fn get_double(&mut self, parameter_index: usize) -> Result<f64, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.double_out_parameter(&parameter))
    }

    /// 读取命名双精度 OUT 参数。
    pub fn get_named_double(&mut self, parameter_name: &str) -> Result<f64, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.double_out_parameter(&parameter))
    }

    /// 读取索引字节 OUT 参数。
    pub fn get_bytes(&mut self, parameter_index: usize) -> Result<Option<Vec<u8>>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.bytes_out_parameter(&parameter))
    }

    /// 读取命名字节 OUT 参数。
    pub fn get_named_bytes(&mut self, parameter_name: &str) -> Result<Option<Vec<u8>>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.bytes_out_parameter(&parameter))
    }

    /// 读取索引 national-character 字符串。
    pub fn get_n_string(&mut self, parameter_index: usize) -> Result<Option<String>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.n_string_out_parameter(&parameter))
    }

    /// 读取命名 national-character 字符串。
    pub fn get_named_n_string(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<String>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.n_string_out_parameter(&parameter))
    }

    /// 读取索引 URL。
    pub fn get_url(&mut self, parameter_index: usize) -> Result<Option<JdbcUrl>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.url_out_parameter(&parameter))
    }

    /// 读取命名 URL。
    pub fn get_named_url(&mut self, parameter_name: &str) -> Result<Option<JdbcUrl>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.url_out_parameter(&parameter))
    }

    /// 读取索引 JDBC `Ref`。
    pub fn get_ref(&mut self, parameter_index: usize) -> Result<Option<JdbcRef>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.ref_out_parameter(&parameter))
    }

    /// 读取命名 JDBC `Ref`。
    pub fn get_named_ref(&mut self, parameter_name: &str) -> Result<Option<JdbcRef>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.ref_out_parameter(&parameter))
    }

    /// 读取索引 JDBC `Array`。
    pub fn get_array(&mut self, parameter_index: usize) -> Result<Option<JdbcArray>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.array_out_parameter(&parameter))
    }

    /// 读取命名 JDBC `Array`。
    pub fn get_named_array(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<JdbcArray>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.array_out_parameter(&parameter))
    }

    /// 读取索引 JDBC `RowId`。
    pub fn get_row_id(&mut self, parameter_index: usize) -> Result<Option<JdbcRowId>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.row_id_out_parameter(&parameter))
    }

    /// 读取命名 JDBC `RowId`。
    pub fn get_named_row_id(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<JdbcRowId>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.row_id_out_parameter(&parameter))
    }

    /// 读取索引 JDBC `SQLXML`。
    pub fn get_sql_xml(
        &mut self,
        parameter_index: usize,
    ) -> Result<Option<JdbcSqlXml>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.sql_xml_out_parameter(&parameter))
    }

    /// 读取命名 JDBC `SQLXML`。
    pub fn get_named_sql_xml(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<JdbcSqlXml>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.sql_xml_out_parameter(&parameter))
    }

    /// 读取索引 Blob OUT 参数。
    ///
    /// 对应 Java：`getBlob(int parameterIndex)`。
    pub fn get_blob(&mut self, parameter_index: usize) -> Result<Option<JdbcBlob>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        let result = self.apply_callable(|statement| statement.blob_out_parameter(&parameter));
        if result.as_ref().is_ok_and(Option::is_some) {
            self.record_blob_open();
        }
        result
    }

    /// 读取命名 Blob OUT 参数。
    ///
    /// 对应 Java：`getBlob(String parameterName)`。
    pub fn get_named_blob(&mut self, parameter_name: &str) -> Result<Option<JdbcBlob>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        let result = self.apply_callable(|statement| statement.blob_out_parameter(&parameter));
        if result.as_ref().is_ok_and(Option::is_some) {
            self.record_blob_open();
        }
        result
    }

    /// 读取索引 Clob OUT 参数。
    pub fn get_clob(&mut self, parameter_index: usize) -> Result<Option<JdbcClob>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        let result = self.apply_callable(|statement| statement.clob_out_parameter(&parameter));
        if result.as_ref().is_ok_and(Option::is_some) {
            self.record_clob_open();
        }
        result
    }

    /// 读取命名 Clob OUT 参数。
    pub fn get_named_clob(&mut self, parameter_name: &str) -> Result<Option<JdbcClob>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        let result = self.apply_callable(|statement| statement.clob_out_parameter(&parameter));
        if result.as_ref().is_ok_and(Option::is_some) {
            self.record_clob_open();
        }
        result
    }

    /// 读取索引 NClob OUT 参数。
    pub fn get_n_clob(&mut self, parameter_index: usize) -> Result<Option<JdbcNClob>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.n_clob_out_parameter(&parameter))
    }

    /// 读取命名 NClob OUT 参数。
    pub fn get_named_n_clob(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<JdbcNClob>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.n_clob_out_parameter(&parameter))
    }

    /// 读取索引普通字符 Reader OUT 参数。
    pub fn get_character_stream(
        &mut self,
        parameter_index: usize,
    ) -> Result<Option<JdbcReader>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.character_stream_out_parameter(&parameter))
    }

    /// 读取命名普通字符 Reader OUT 参数。
    pub fn get_named_character_stream(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.character_stream_out_parameter(&parameter))
    }

    /// 读取索引 national character Reader OUT 参数。
    pub fn get_n_character_stream(
        &mut self,
        parameter_index: usize,
    ) -> Result<Option<JdbcReader>, DruidError> {
        let parameter = self.index_parameter(parameter_index)?;
        self.apply_callable(|statement| statement.n_character_stream_out_parameter(&parameter))
    }

    /// 读取命名 national character Reader OUT 参数。
    pub fn get_named_n_character_stream(
        &mut self,
        parameter_name: &str,
    ) -> Result<Option<JdbcReader>, DruidError> {
        let parameter = self.named_parameter(parameter_name)?;
        self.apply_callable(|statement| statement.n_character_stream_out_parameter(&parameter))
    }

    fn get_date_with_argument(
        &mut self,
        parameter: Result<CallableParameter, DruidError>,
        calendar: CallableCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        let parameter = self.record_result(parameter)?;
        self.apply_callable(|statement| statement.date_out_parameter(&parameter, &calendar))
    }

    fn get_time_with_argument(
        &mut self,
        parameter: Result<CallableParameter, DruidError>,
        calendar: CallableCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        let parameter = self.record_result(parameter)?;
        self.apply_callable(|statement| statement.time_out_parameter(&parameter, &calendar))
    }

    fn get_timestamp_with_argument(
        &mut self,
        parameter: Result<CallableParameter, DruidError>,
        calendar: CallableCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        let parameter = self.record_result(parameter)?;
        self.apply_callable(|statement| statement.timestamp_out_parameter(&parameter, &calendar))
    }

    fn register(
        &mut self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), DruidError> {
        self.apply_callable(|statement| statement.register_out_parameter(parameter, out_parameter))
    }

    fn get(&mut self, parameter: CallableParameter) -> Result<JdbcObject, DruidError> {
        let result = self.apply_callable(|statement| statement.out_parameter(&parameter));
        self.record_object_lob_result(&result);
        result
    }

    fn apply_callable<T>(
        &mut self,
        operation: impl FnOnce(&dyn PhysicalCallableStatement) -> Result<T, DruidError>,
    ) -> Result<T, DruidError> {
        let result = self.physical_callable_statement().and_then(operation);
        if result.is_err() {
            self.prepared_statement.record_exception();
        }
        result
    }

    fn index_parameter(&mut self, parameter_index: usize) -> Result<CallableParameter, DruidError> {
        let result = CallableParameter::by_index(parameter_index);
        self.record_result(result)
    }

    fn named_parameter(
        &mut self,
        parameter_name: impl Into<String>,
    ) -> Result<CallableParameter, DruidError> {
        let result = CallableParameter::by_name(parameter_name);
        self.record_result(result)
    }

    fn record_result<T>(&mut self, result: Result<T, DruidError>) -> Result<T, DruidError> {
        if result.is_err() {
            self.prepared_statement.record_exception();
        }
        result
    }

    fn record_blob_open(&self) {
        self.prepared_statement
            .pooled_statement()
            .record_blob_open();
    }

    fn record_object_lob_result(&self, result: &Result<JdbcObject, DruidError>) {
        let Ok(value) = result.as_ref() else {
            return;
        };
        match value {
            JdbcObject::Blob(_) => self.record_blob_open(),
            JdbcObject::Clob(_) | JdbcObject::NClob(_) => self.record_clob_open(),
            _ => {}
        }
    }

    fn record_clob_open(&self) {
        self.prepared_statement
            .pooled_statement()
            .record_clob_open();
    }
}
