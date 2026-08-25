//! Toasty 物理连接适配器。

use super::ToastyPreparedStatement;
use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{owned_executor, AdapterExecutionCore, StandardExecutor};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::{BatchUpdateCounts, RdbcError};
use druid_rdbc::error::{SqlException, SqlWarning};
use druid_rdbc::resource::{RdbcCharacterLength, RdbcInputStream, RdbcReader, RdbcStreamLength};
use druid_rdbc::spi::{
    ExecResult, PhysicalConnection, PhysicalConnectionCapabilities, PhysicalDatabaseMetaData,
    PhysicalPreparedStatement, PhysicalResultSet, PreparedInputParameter, PreparedStatementKey,
    RowSetResultSet, StatementExecuteResult, StatementGeneratedKeys,
};
use druid_rdbc::types::{RdbcObject, Row, Savepoint, Value};
use std::sync::Arc;
use toasty_core::{
    driver::{
        operation::{
            IsolationLevel, Operation, RawSql, RawSqlRet, Transaction, TransactionMode, TypedValue,
        },
        Capability, Connection as ToastyConnection, Rows,
    },
    schema::db,
    stmt::Value as ToastyValue,
    Schema,
};

fn parse_naive_datetime(value: &str) -> Result<Value, RdbcError> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f"))
        .map(Value::Timestamp)
        .map_err(|error| RdbcError::DriverError(error.to_string()))
}

fn read_only_session_sql(driver_name: &str, read_only: bool) -> Option<&'static str> {
    if driver_name.eq_ignore_ascii_case("postgresql") {
        Some(if read_only {
            "SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY"
        } else {
            "SET SESSION CHARACTERISTICS AS TRANSACTION READ WRITE"
        })
    } else if driver_name.eq_ignore_ascii_case("mysql") {
        Some(if read_only {
            "SET SESSION TRANSACTION READ ONLY"
        } else {
            "SET SESSION TRANSACTION READ WRITE"
        })
    } else {
        None
    }
}

/// 将一个未池化 Toasty driver connection 适配为 Druid 物理连接。
///
/// 对应 Java 平台对象：RDBC driver 的 `java.sql.Connection` 实现。对象不持有
/// `toasty::Db` 或 Toasty Pool；DruidPool 独占池化、回收和统计职责。
pub struct ToastyConnectionAdapter {
    connection: Option<Box<dyn ToastyConnection>>,
    schema: Arc<Schema>,
    url: String,
    driver_name: &'static str,
    auto_commit: bool,
    read_only: bool,
    isolation: Option<IsolationLevel>,
    savepoint_sequence: u64,
    discarded: bool,
    standard_executor: Option<Arc<dyn StandardExecutor>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl std::fmt::Debug for ToastyConnectionAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToastyConnectionAdapter")
            .field("driver_name", &self.driver_name)
            .field("auto_commit", &self.auto_commit)
            .field("read_only", &self.read_only)
            .field("isolation", &self.isolation)
            .field("discarded", &self.discarded)
            .field("closed", &self.connection.is_none())
            .finish_non_exhaustive()
    }
}

impl ToastyConnectionAdapter {
    pub(crate) fn new(
        connection: Box<dyn ToastyConnection>,
        schema: Arc<Schema>,
        capability: &'static Capability,
        url: String,
    ) -> Self {
        Self {
            connection: Some(connection),
            schema,
            url,
            driver_name: capability.driver_name,
            auto_commit: true,
            read_only: false,
            isolation: None,
            savepoint_sequence: 0,
            discarded: false,
            standard_executor: None,
            runtime_trace: None,
        }
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    fn standard_executor_core(&mut self) -> Result<Arc<dyn StandardExecutor>, RdbcError> {
        if let Some(executor) = &self.standard_executor {
            return Ok(Arc::clone(executor));
        }
        self.connection_mut()?;
        let runtime_trace = self.runtime_trace.take();
        let shell = Self {
            connection: None,
            schema: Arc::clone(&self.schema),
            url: self.url.clone(),
            driver_name: self.driver_name,
            auto_commit: self.auto_commit,
            read_only: self.read_only,
            isolation: self.isolation,
            savepoint_sequence: self.savepoint_sequence,
            discarded: self.discarded,
            standard_executor: None,
            runtime_trace: None,
        };
        let owned = std::mem::replace(self, shell);
        let executor = owned_executor(owned, runtime_trace);
        self.standard_executor = Some(Arc::clone(&executor));
        Ok(executor)
    }

    async fn execute_update_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<ExecResult, RdbcError> {
        let response = self
            .execute_operation(Self::raw_sql(sql, params, RawSqlRet::None)?)
            .await?;
        let Rows::Count(rows_affected) = response.values else {
            return Err(RdbcError::DriverError(
                "Toasty statement returned rows instead of an update count".to_string(),
            ));
        };
        let last_insert_id = self.sqlite_last_insert_id().await?;
        Ok(ExecResult {
            rows_affected,
            last_insert_id,
            row_count: None,
        })
    }

    async fn execute_query_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, RdbcError> {
        let response = self
            .execute_operation(Self::raw_sql(sql, params, RawSqlRet::Infer)?)
            .await?;
        let values = response
            .values
            .collect_as_value()
            .await
            .map_err(|error| Self::driver_error(&error))?;
        let ToastyValue::List(rows) = values else {
            return Err(RdbcError::DriverError(format!(
                "Toasty query must return a row list, actual={values:?}"
            )));
        };
        rows.into_iter().map(Self::row).collect()
    }

    #[allow(
        clippy::unused_async,
        reason = "uniform async close core across adapter families"
    )]
    async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.connection.take();
        self.auto_commit = true;
        Ok(())
    }

    fn connection_mut(&mut self) -> Result<&mut Box<dyn ToastyConnection>, RdbcError> {
        if self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.connection
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    fn driver_error(error: &toasty_core::Error) -> RdbcError {
        if error.is_connection_lost() {
            RdbcError::ConnectionDiscarded
        } else if error.is_driver_operation_failed() {
            // Toasty 0.9 不公开底层驱动的 vendor code/SQLState，但明确区分
            // driver operation failure。保留这一结构化类别与完整消息，不能把
            // 数据库执行错误降级成普通字符串错误。
            RdbcError::SqlException(Box::new(
                SqlException::driver(0, error.to_string())
                    .with_class_name("toasty_core::error::DriverOperationFailed"),
            ))
        } else {
            RdbcError::DriverError(error.to_string())
        }
    }

    async fn execute_operation(
        &mut self,
        operation: Operation,
    ) -> Result<toasty_core::driver::ExecResponse, RdbcError> {
        let schema = Arc::clone(&self.schema);
        self.connection_mut()?
            .exec(&schema, operation)
            .await
            .map_err(|error| Self::driver_error(&error))
    }

    fn typed_parameter(value: Value) -> Result<TypedValue, RdbcError> {
        Ok(match value {
            // Druid Value 目前不携带 RDBC targetSqlType；与现有 SQLx/RBDC
            // Adapter 一致，未定型 null 使用通用文本 storage type。
            Value::Null => TypedValue {
                value: ToastyValue::Null,
                ty: db::Type::Text,
            },
            Value::Bool(value) => TypedValue {
                value: ToastyValue::Bool(value),
                ty: db::Type::Boolean,
            },
            Value::Int(value) => TypedValue {
                value: ToastyValue::I64(value),
                ty: db::Type::Integer(8),
            },
            Value::Float(value) => TypedValue {
                value: ToastyValue::F64(value),
                ty: db::Type::Float(8),
            },
            Value::Decimal(value) => TypedValue {
                value: ToastyValue::BigDecimal(value),
                ty: db::Type::Numeric(None),
            },
            Value::Date(value) => TypedValue {
                value: ToastyValue::Date(
                    value
                        .format("%Y-%m-%d")
                        .to_string()
                        .parse::<jiff::civil::Date>()
                        .map_err(|error| RdbcError::DriverError(error.to_string()))?,
                ),
                ty: db::Type::Date,
            },
            Value::Time(value) => TypedValue {
                value: ToastyValue::Time(
                    value
                        .format("%H:%M:%S%.f")
                        .to_string()
                        .parse::<jiff::civil::Time>()
                        .map_err(|error| RdbcError::DriverError(error.to_string()))?,
                ),
                ty: db::Type::Time(9),
            },
            Value::Timestamp(value) => TypedValue {
                value: ToastyValue::DateTime(
                    value
                        .format("%Y-%m-%dT%H:%M:%S%.f")
                        .to_string()
                        .parse::<jiff::civil::DateTime>()
                        .map_err(|error| RdbcError::DriverError(error.to_string()))?,
                ),
                ty: db::Type::DateTime(9),
            },
            Value::String(value) => TypedValue {
                value: ToastyValue::String(value),
                ty: db::Type::Text,
            },
            Value::Bytes(value) => TypedValue {
                value: ToastyValue::Bytes(value),
                ty: db::Type::Blob,
            },
        })
    }

    fn raw_sql(sql: &str, params: Vec<Value>, ret: RawSqlRet) -> Result<Operation, RdbcError> {
        Ok(RawSql {
            sql: sql.to_string(),
            params: params
                .into_iter()
                .map(Self::typed_parameter)
                .collect::<Result<Vec<_>, _>>()?,
            ret,
        }
        .into())
    }

    fn stream_length(length: RdbcStreamLength) -> Result<Option<usize>, RdbcError> {
        match length {
            RdbcStreamLength::Unspecified => Ok(None),
            RdbcStreamLength::Int(length) => usize::try_from(length).map(Some).map_err(|_| {
                RdbcError::InvalidArgument("stream length must not be negative".to_string())
            }),
            RdbcStreamLength::Long(length) => usize::try_from(length).map(Some).map_err(|_| {
                RdbcError::InvalidArgument(
                    "stream length must be non-negative and fit usize".to_string(),
                )
            }),
        }
    }

    fn character_length(length: RdbcCharacterLength) -> Result<Option<usize>, RdbcError> {
        match length {
            RdbcCharacterLength::Unspecified => Ok(None),
            RdbcCharacterLength::Int(length) => usize::try_from(length).map(Some).map_err(|_| {
                RdbcError::InvalidArgument("reader length must not be negative".to_string())
            }),
            RdbcCharacterLength::Long(length) => usize::try_from(length).map(Some).map_err(|_| {
                RdbcError::InvalidArgument(
                    "reader length must be non-negative and fit usize".to_string(),
                )
            }),
        }
    }

    fn read_stream(
        stream: &RdbcInputStream,
        length: RdbcStreamLength,
    ) -> Result<Vec<u8>, RdbcError> {
        let Some(length) = Self::stream_length(length)? else {
            return stream.read_to_end();
        };
        let mut bytes = vec![0_u8; length];
        let mut offset = 0;
        while offset < length {
            let read = stream.read(&mut bytes[offset..])?;
            if read == 0 {
                return Err(RdbcError::DriverError(format!(
                    "InputStream ended after {offset} bytes; declared length is {length}"
                )));
            }
            offset += read;
        }
        Ok(bytes)
    }

    fn read_reader(reader: &RdbcReader, length: RdbcCharacterLength) -> Result<String, RdbcError> {
        let Some(length) = Self::character_length(length)? else {
            return reader.read_to_string();
        };
        let mut code_units = vec![0_u16; length];
        let mut offset = 0;
        while offset < length {
            let read = reader.read_utf16(&mut code_units[offset..])?;
            if read == 0 {
                return Err(RdbcError::DriverError(format!(
                    "Reader ended after {offset} UTF-16 units; declared length is {length}"
                )));
            }
            offset += read;
        }
        String::from_utf16(&code_units).map_err(|error| {
            RdbcError::DriverError(format!("Reader contains invalid UTF-16: {error}"))
        })
    }

    fn immediate_rdbc_object_parameter(value: &RdbcObject) -> Result<Value, RdbcError> {
        match value {
            RdbcObject::RowId(value) => Ok(Value::Bytes(value.bytes().to_vec())),
            RdbcObject::CharacterStream(value) | RdbcObject::NCharacterStream(value) => {
                Self::read_reader(value, RdbcCharacterLength::Unspecified).map(Value::String)
            }
            _ => PreparedInputParameter::object(Some(value.clone())).scalar_value(),
        }
    }

    async fn deferred_rdbc_object_parameter(value: &RdbcObject) -> Result<Value, RdbcError> {
        match value {
            RdbcObject::SqlXml(value) => value.string().await?.to_rust_string().map(Value::String),
            RdbcObject::Blob(value) => {
                let length = value.length().await?;
                let length = i32::try_from(length).map_err(|_| {
                    RdbcError::InvalidArgument(
                        "Blob length exceeds RDBC getBytes int range".to_string(),
                    )
                })?;
                value.get_bytes(1, length).await.map(Value::Bytes)
            }
            RdbcObject::Clob(value) => {
                let length = value.length().await?;
                let length = i32::try_from(length).map_err(|_| {
                    RdbcError::InvalidArgument(
                        "Clob length exceeds RDBC getSubString int range".to_string(),
                    )
                })?;
                value
                    .get_sub_string(1, length)
                    .await?
                    .to_rust_string()
                    .map(Value::String)
            }
            RdbcObject::NClob(value) => {
                let length = value.length().await?;
                let length = i32::try_from(length).map_err(|_| {
                    RdbcError::InvalidArgument(
                        "NClob length exceeds RDBC getSubString int range".to_string(),
                    )
                })?;
                value
                    .get_sub_string(1, length)
                    .await?
                    .to_rust_string()
                    .map(Value::String)
            }
            _ => Self::immediate_rdbc_object_parameter(value),
        }
    }

    pub(super) fn prepared_parameter_immediate(
        parameter: &PreparedInputParameter,
    ) -> Result<Option<Value>, RdbcError> {
        if matches!(
            parameter,
            PreparedInputParameter::Blob(Some(_))
                | PreparedInputParameter::Clob(Some(_))
                | PreparedInputParameter::NClob(Some(_))
                | PreparedInputParameter::SqlXml(Some(_))
                | PreparedInputParameter::Object {
                    value: Some(
                        RdbcObject::Blob(_)
                            | RdbcObject::Clob(_)
                            | RdbcObject::NClob(_)
                            | RdbcObject::SqlXml(_)
                    ),
                    ..
                }
        ) {
            return Ok(None);
        }

        match parameter {
            PreparedInputParameter::AsciiStream { stream, length } => stream
                .as_ref()
                .map(|stream| {
                    let bytes = Self::read_stream(stream, *length)?;
                    String::from_utf8(bytes)
                        .map(Value::String)
                        .map_err(|error| {
                            RdbcError::DriverError(format!(
                                "ASCII stream is not valid UTF-8: {error}"
                            ))
                        })
                })
                .transpose()
                .map(|value| Some(value.unwrap_or(Value::Null))),
            PreparedInputParameter::UnicodeStream { stream, length } => stream
                .as_ref()
                .map(|stream| {
                    let length = RdbcStreamLength::Int(*length);
                    let bytes = Self::read_stream(stream, length)?;
                    String::from_utf8(bytes)
                        .map(Value::String)
                        .map_err(|error| {
                            RdbcError::DriverError(format!(
                                "Unicode stream is not valid UTF-8: {error}"
                            ))
                        })
                })
                .transpose()
                .map(|value| Some(value.unwrap_or(Value::Null))),
            PreparedInputParameter::BinaryStream { stream, length }
            | PreparedInputParameter::BlobStream { stream, length } => stream
                .as_ref()
                .map(|stream| Self::read_stream(stream, *length).map(Value::Bytes))
                .transpose()
                .map(|value| Some(value.unwrap_or(Value::Null))),
            PreparedInputParameter::CharacterStream { reader, length }
            | PreparedInputParameter::NCharacterStream { reader, length }
            | PreparedInputParameter::ClobReader { reader, length }
            | PreparedInputParameter::NClobReader { reader, length } => reader
                .as_ref()
                .map(|reader| Self::read_reader(reader, *length).map(Value::String))
                .transpose()
                .map(|value| Some(value.unwrap_or(Value::Null))),
            PreparedInputParameter::RowId(Some(value)) => {
                Ok(Some(Value::Bytes(value.bytes().to_vec())))
            }
            PreparedInputParameter::Object {
                value: Some(value), ..
            } => Self::immediate_rdbc_object_parameter(value).map(Some),
            _ => parameter.scalar_value().map(Some),
        }
    }

    pub(super) async fn prepared_parameter(
        parameter: &PreparedInputParameter,
    ) -> Result<Value, RdbcError> {
        if let Some(value) = Self::prepared_parameter_immediate(parameter)? {
            return Ok(value);
        }

        match parameter {
            PreparedInputParameter::Blob(Some(value)) => {
                Self::deferred_rdbc_object_parameter(&RdbcObject::Blob(value.clone())).await
            }
            PreparedInputParameter::Clob(Some(value)) => {
                Self::deferred_rdbc_object_parameter(&RdbcObject::Clob(value.clone())).await
            }
            PreparedInputParameter::NClob(Some(value)) => {
                Self::deferred_rdbc_object_parameter(&RdbcObject::NClob(value.clone())).await
            }
            PreparedInputParameter::SqlXml(Some(value)) => {
                value.string().await?.to_rust_string().map(Value::String)
            }
            PreparedInputParameter::Object {
                value: Some(value), ..
            } => Self::deferred_rdbc_object_parameter(value).await,
            _ => unreachable!("immediate parameters already returned"),
        }
    }

    async fn converted_prepared_parameters(
        parameters: &[PreparedInputParameter],
    ) -> Result<Vec<Value>, RdbcError> {
        let mut values = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            values.push(Self::prepared_parameter(parameter).await?);
        }
        Ok(values)
    }

    async fn prepared_parameters(
        statement: &dyn PhysicalPreparedStatement,
        parameters: &[PreparedInputParameter],
    ) -> Result<Vec<Value>, RdbcError> {
        if let Some(statement) = statement.as_any().downcast_ref::<ToastyPreparedStatement>() {
            statement.materialized_parameters(parameters.len()).await
        } else {
            Self::converted_prepared_parameters(parameters).await
        }
    }

    fn value(value: ToastyValue) -> Result<Value, RdbcError> {
        match value {
            ToastyValue::Null => Ok(Value::Null),
            ToastyValue::Bool(value) => Ok(Value::Bool(value)),
            ToastyValue::I8(value) => Ok(Value::Int(i64::from(value))),
            ToastyValue::I16(value) => Ok(Value::Int(i64::from(value))),
            ToastyValue::I32(value) => Ok(Value::Int(i64::from(value))),
            ToastyValue::I64(value) => Ok(Value::Int(value)),
            ToastyValue::U8(value) => Ok(Value::Int(i64::from(value))),
            ToastyValue::U16(value) => Ok(Value::Int(i64::from(value))),
            ToastyValue::U32(value) => Ok(Value::Int(i64::from(value))),
            ToastyValue::U64(value) => i64::try_from(value).map(Value::Int).map_err(|_| {
                RdbcError::DriverError(
                    "Toasty u64 result cannot be represented by crate::core::Value::Int"
                        .to_string(),
                )
            }),
            ToastyValue::F32(value) => Ok(Value::Float(f64::from(value))),
            ToastyValue::F64(value) => Ok(Value::Float(value)),
            ToastyValue::BigDecimal(value) => Ok(Value::Decimal(value)),
            ToastyValue::Date(value) => NaiveDate::parse_from_str(&value.to_string(), "%Y-%m-%d")
                .map(Value::Date)
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            ToastyValue::Time(value) => {
                NaiveTime::parse_from_str(&format!("{value:.9}"), "%H:%M:%S%.f")
                    .map(Value::Time)
                    .map_err(|error| RdbcError::DriverError(error.to_string()))
            }
            ToastyValue::DateTime(value) => parse_naive_datetime(&format!("{value:.9}")),
            ToastyValue::Timestamp(value) => DateTime::parse_from_rfc3339(&value.to_string())
                .map(|value| Value::Timestamp(value.naive_utc()))
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            ToastyValue::String(value) => Ok(Value::String(value)),
            ToastyValue::Bytes(value) => Ok(Value::Bytes(value)),
            unsupported => Err(RdbcError::DriverError(format!(
                "Toasty value {unsupported:?} is not represented by crate::core::Value"
            ))),
        }
    }

    fn row(value: ToastyValue) -> Result<Row, RdbcError> {
        let ToastyValue::Record(record) = value else {
            return Err(RdbcError::DriverError(format!(
                "Toasty raw SQL row must be Value::Record, actual={value:?}"
            )));
        };
        record
            .into_iter()
            .map(Self::value)
            .collect::<Result<Vec<_>, _>>()
            .map(Row::new)
    }

    fn validate_savepoint_name(name: &str) -> Result<(), RdbcError> {
        let valid = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(())
        } else {
            Err(RdbcError::InvalidArgument(
                "savepoint name must contain only ASCII letters, digits, or '_'".to_string(),
            ))
        }
    }

    fn savepoint_name(savepoint: &Savepoint) -> String {
        savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id))
    }

    fn is_sqlite(&self) -> bool {
        self.driver_name.eq_ignore_ascii_case("sqlite")
    }

    fn ensure_transaction(&self) -> Result<(), RdbcError> {
        if self.auto_commit {
            Err(RdbcError::DriverError(
                "transaction operation requires auto_commit=false".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    async fn sqlite_last_insert_id(&mut self) -> Result<Option<i64>, RdbcError> {
        if !self.is_sqlite() {
            return Ok(None);
        }
        let response = self
            .execute_operation(Self::raw_sql(
                "SELECT last_insert_rowid()",
                Vec::new(),
                RawSqlRet::Infer,
            )?)
            .await?;
        let values = response
            .values
            .collect_as_value()
            .await
            .map_err(|error| Self::driver_error(&error))?;
        let ToastyValue::List(mut rows) = values else {
            return Err(RdbcError::DriverError(format!(
                "SQLite last_insert_rowid() must return a row list, actual={values:?}"
            )));
        };
        let Some(ToastyValue::Record(mut row)) = rows.pop() else {
            return Ok(None);
        };
        match row.fields.pop() {
            Some(ToastyValue::I64(0)) | None => Ok(None),
            Some(ToastyValue::I64(value)) => Ok(Some(value)),
            value => Err(RdbcError::DriverError(format!(
                "SQLite last_insert_rowid() must return i64, actual={value:?}"
            ))),
        }
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for ToastyConnectionAdapter {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        self.execute_update_core(sql, params).await
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, RdbcError> {
        if matches!(
            generated_keys,
            StatementGeneratedKeys::ColumnIndexes(_) | StatementGeneratedKeys::ColumnNames(_)
        ) {
            // xerial SQLite 对这两个 RDBC 重载抛 SQLFeatureNotSupportedException；
            // Toasty 0.9 也没有向驱动传递列选择的契约，不能静默忽略参数。
            return Err(RdbcError::UnsupportedOperation {
                operation: "statement_execute_generated_key_columns",
            });
        }

        let response = self
            .execute_operation(Self::raw_sql(sql, params, RawSqlRet::Infer)?)
            .await?;
        match response.values {
            Rows::Count(rows_affected) => {
                let last_insert_id = self.sqlite_last_insert_id().await?;
                Ok(vec![StatementExecuteResult::Update(ExecResult {
                    rows_affected,
                    last_insert_id,
                    row_count: None,
                })])
            }
            values => {
                let values = values
                    .collect_as_value()
                    .await
                    .map_err(|error| Self::driver_error(&error))?;
                let ToastyValue::List(rows) = values else {
                    return Err(RdbcError::DriverError(format!(
                        "Toasty generic execute query must return a row list, actual={values:?}"
                    )));
                };
                let rows = rows
                    .into_iter()
                    .map(Self::row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(vec![StatementExecuteResult::ResultSet(rows)])
            }
        }
    }

    async fn execute_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        _generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, RdbcError> {
        if statement.is_closed() {
            return Err(RdbcError::ConnectionDiscarded);
        }
        // xerial SQLite 的 PreparedStatement column-index/column-name 重载在
        // prepare 时接受参数，并在 execute 时返回 rowid；Toasty SQLite 没有
        // 可选择的多列 generated-key 描述，因此按同一单 rowid 语义执行。
        // 普通 Statement 的对应 execute 重载仍保持 xerial 的 unsupported。
        self.execute(statement.sql(), params, StatementGeneratedKeys::None)
            .await
    }

    async fn exec_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<ExecResult, RdbcError> {
        let params = Self::prepared_parameters(statement, &parameters).await?;
        self.exec_prepared(statement, params).await
    }

    async fn execute_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, RdbcError> {
        let params = Self::prepared_parameters(statement, &parameters).await?;
        self.execute_prepared(statement, params, generated_keys)
            .await
    }

    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i32>, RdbcError> {
        if statement.is_closed() {
            return Err(RdbcError::ConnectionDiscarded);
        }

        let materialized_batches =
            if let Some(statement) = statement.as_any().downcast_ref::<ToastyPreparedStatement>() {
                statement.take_batches(parameter_sets.len()).await?
            } else {
                None
            };
        let parameter_sets = if let Some(materialized_batches) = materialized_batches {
            materialized_batches
        } else {
            let mut materialized = Vec::with_capacity(parameter_sets.len());
            for parameters in &parameter_sets {
                materialized.push(
                    Self::converted_prepared_parameters(parameters)
                        .await
                        .map_err(|error| RdbcError::BatchUpdateException {
                            update_counts: Vec::<i32>::new().into(),
                            cause: Box::new(error),
                        })?,
                );
            }
            materialized
        };
        let mut update_counts = BatchUpdateCounts::default();
        for params in parameter_sets {
            match self.exec_prepared(statement, params).await {
                Ok(result) => {
                    if let Err(cause) = update_counts.try_push_rows_affected(result.rows_affected) {
                        return Err(RdbcError::BatchUpdateException {
                            update_counts,
                            cause: Box::new(cause),
                        });
                    }
                }
                Err(error) => {
                    return Err(RdbcError::BatchUpdateException {
                        update_counts,
                        cause: Box::new(error),
                    });
                }
            }
        }
        Ok(update_counts.jdbc_i32())
    }

    async fn fetch_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Vec<Row>, RdbcError> {
        let params = Self::prepared_parameters(statement, &parameters).await?;
        self.fetch_prepared(statement, params).await
    }

    async fn fetch_prepared_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, RdbcError> {
        let rows = self.fetch_prepared(statement, params).await?;
        Ok(Arc::new(RowSetResultSet::new(rows)))
    }

    async fn fetch_prepared_parameters_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Arc<dyn PhysicalResultSet>, RdbcError> {
        let params = Self::prepared_parameters(statement, &parameters).await?;
        self.fetch_prepared_result_set(statement, params).await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        self.execute_query_core(sql, params).await
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, RdbcError> {
        let _ = self.connection_mut()?;
        // Toasty SQL drivers prepare/cache raw SQL inside Connection#exec。
        // Druid holder 保留逻辑句柄及缓存命中，执行仍走同一物理连接。
        Ok(Arc::new(ToastyPreparedStatement::new(key.sql())))
    }

    async fn begin(&mut self) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::DriverError(
                "Toasty transaction is already active".to_string(),
            ));
        }
        self.execute_operation(
            Transaction::Start {
                isolation: self.isolation,
                read_only: self.read_only,
                mode: TransactionMode::Default,
            }
            .into(),
        )
        .await?;
        self.auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.ensure_transaction()?;
        self.execute_operation(Transaction::Commit.into()).await?;
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.ensure_transaction()?;
        self.execute_operation(Transaction::Rollback.into()).await?;
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.ensure_transaction()?;
        let name = Self::savepoint_name(savepoint);
        Self::validate_savepoint_name(&name)?;
        self.execute_operation(Transaction::RollbackToSavepoint(name).into())
            .await?;
        Ok(())
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        self.ensure_transaction()?;
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        let savepoint = Savepoint {
            id: self.savepoint_sequence,
            name: None,
        };
        let name = Self::savepoint_name(&savepoint);
        self.execute_operation(Transaction::Savepoint(name).into())
            .await?;
        Ok(savepoint)
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, RdbcError> {
        self.ensure_transaction()?;
        Self::validate_savepoint_name(name)?;
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        self.execute_operation(Transaction::Savepoint(name.to_string()).into())
            .await?;
        Ok(Savepoint {
            id: self.savepoint_sequence,
            name: Some(name.to_string()),
        })
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), RdbcError> {
        self.ensure_transaction()?;
        let name = Self::savepoint_name(savepoint);
        Self::validate_savepoint_name(&name)?;
        self.execute_operation(Transaction::ReleaseSavepoint(name).into())
            .await?;
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        self.connection_mut()?
            .ping()
            .await
            .map_err(|error| Self::driver_error(&error))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        // Toasty Connection SPI 以 Drop 关闭 raw driver connection。
        // SQLite/rusqlite 与网络驱动都会在 drop 时回滚未提交事务。
        self.close_connection_core().await
    }

    fn is_closed(&self) -> bool {
        self.connection.is_none()
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        PhysicalConnectionCapabilities {
            transactions: true,
            savepoints: true,
            auto_commit: true,
            read_only: read_only_session_sql(self.driver_name, true).is_some(),
            transaction_isolation: true,
            holdability: false,
            clear_warnings: true,
            catalog: false,
            schema: false,
        }
    }

    fn database_meta_data(&mut self) -> Result<Box<dyn PhysicalDatabaseMetaData + '_>, RdbcError> {
        self.connection_mut()?;
        Ok(Box::new(
            super::toasty_database_meta_data::ToastyDatabaseMetaData::new(
                &self.url,
                self.driver_name,
                self.read_only,
            ),
        ))
    }

    fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), RdbcError> {
        match (self.auto_commit, auto_commit) {
            (true, false) => self.begin().await,
            (false, true) => self.commit().await,
            _ => Ok(()),
        }
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::InvalidArgument(
                "read_only cannot change while a transaction is active".to_string(),
            ));
        }
        self.connection_mut()?;
        let Some(sql) = read_only_session_sql(self.driver_name, read_only) else {
            if read_only {
                return Err(RdbcError::UnsupportedOperation {
                    operation: if self.is_sqlite() {
                        "toasty_sqlite_read_only_transaction"
                    } else {
                        "toasty_driver_read_only_transaction"
                    },
                });
            }
            self.read_only = false;
            return Ok(());
        };
        self.execute_operation(Self::raw_sql(sql, Vec::new(), RawSqlRet::None)?)
            .await?;
        self.read_only = read_only;
        Ok(())
    }

    fn transaction_isolation(&self) -> u8 {
        match self.isolation {
            Some(IsolationLevel::ReadUncommitted) => 1,
            Some(IsolationLevel::RepeatableRead) => 4,
            Some(IsolationLevel::Serializable) => 8,
            None if self.driver_name.eq_ignore_ascii_case("sqlite") => 8,
            Some(IsolationLevel::ReadCommitted) | None => 2,
        }
    }

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::InvalidArgument(
                "transaction isolation cannot change while a transaction is active".to_string(),
            ));
        }
        if self.is_sqlite() && level != 8 {
            return Err(RdbcError::InvalidArgument(
                "Toasty SQLite only supports RDBC TRANSACTION_SERIALIZABLE (8)".to_string(),
            ));
        }
        self.isolation = Some(match level {
            1 => IsolationLevel::ReadUncommitted,
            2 => IsolationLevel::ReadCommitted,
            4 => IsolationLevel::RepeatableRead,
            8 => IsolationLevel::Serializable,
            _ => {
                return Err(RdbcError::InvalidArgument(format!(
                    "unsupported RDBC transaction isolation level: {level}"
                )));
            }
        });
        Ok(())
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        if self.is_closed() {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(None)
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        if self.is_closed() {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(())
    }

    fn mark_discarded(&mut self) {
        self.discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.discarded
    }

    fn driver_name(&self) -> &str {
        self.driver_name
    }
}

#[async_trait::async_trait]
impl AdapterExecutionCore for ToastyConnectionAdapter {
    async fn standard_execute_update_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<i64, RdbcError> {
        let result = self.execute_update_core(sql, values).await?;
        i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".to_owned())
        })
    }

    async fn standard_execute_query_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError> {
        self.execute_query_core(sql, values)
            .await
            .map(StandardQueryResult::rows_only)
    }

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.close_connection_core().await
    }
}

druid_rdbc::impl_standard_wrapper!(ToastyConnectionAdapter => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for ToastyConnectionAdapter {
    async fn create_statement(
        &mut self,
        _options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Ok(Box::new(StandardAdapterStatement::new(
            self.standard_executor_core()?,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        _options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let parameter_count = executor.parameter_count(sql).await?;
        Ok(Box::new(StandardAdapterPreparedStatement::new(
            sql.to_owned(),
            executor,
            parameter_count,
        )))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        let mut snapshot = super::ToastyDatabaseMetaData::owned(
            self.url.clone(),
            self.driver_name.to_owned(),
            self.read_only,
        );
        let product_name =
            druid_rdbc::metadata::DatabaseMetaData::database_product_name(&mut snapshot)
                .await
                .ok();
        Ok(Box::new(StandardAdapterDatabaseMetaData::new(
            self.standard_executor_core()?,
            product_name,
            "toasty_database_metadata_product_name",
        )))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if let Some(executor) = &self.standard_executor {
            executor.close().await
        } else {
            self.close_connection_core().await
        }
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.standard_executor.as_ref().map_or_else(
            || self.connection.is_none(),
            |executor| executor.is_closed(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::read_only_session_sql;

    #[test]
    fn read_only_session_sql_is_backend_specific() {
        assert_eq!(
            read_only_session_sql("postgresql", true),
            Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
        );
        assert_eq!(
            read_only_session_sql("mysql", false),
            Some("SET SESSION TRANSACTION READ WRITE")
        );
        assert_eq!(read_only_session_sql("sqlite", true), None);
        assert_eq!(read_only_session_sql("turso", true), None);
    }
}
