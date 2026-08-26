//! Toasty canonical 连接适配器。

use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{
    owned_executor, AdapterExecutionCore, AdapterUpdateResult, StandardExecutor,
};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use druid_rdbc::types::{Row, Value};
use std::sync::Arc;
use toasty_core::{
    driver::{
        operation::{IsolationLevel, Operation, RawSql, RawSqlRet, TypedValue},
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

#[cfg(test)]
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
    ) -> Result<AdapterUpdateResult, RdbcError> {
        let response = self
            .execute_operation(Self::raw_sql(sql, params, RawSqlRet::None)?)
            .await?;
        let Rows::Count(rows_affected) = response.values else {
            return Err(RdbcError::DriverError(
                "Toasty statement returned rows instead of an update count".to_string(),
            ));
        };
        let last_insert_id = self.sqlite_last_insert_id().await?;
        Ok(AdapterUpdateResult {
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

    fn is_sqlite(&self) -> bool {
        self.driver_name.eq_ignore_ascii_case("sqlite")
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
