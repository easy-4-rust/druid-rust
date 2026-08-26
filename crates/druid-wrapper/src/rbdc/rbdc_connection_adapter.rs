//! RBDC canonical 连接适配器。

use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{
    owned_executor, AdapterExecutionCore, AdapterUpdateResult, StandardExecutor,
};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use bigdecimal::BigDecimal;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use druid_rdbc::types::{Row, Value};
use futures::StreamExt;
use std::sync::Arc;

fn parse_rbdc_string(value: rbs::Value, type_name: &str) -> Result<String, RdbcError> {
    match value {
        rbs::Value::String(value) => Ok(value),
        actual => Err(RdbcError::DriverError(format!(
            "RBDC {type_name} extension must contain String, got {actual:?}"
        ))),
    }
}

fn parse_rbdc_datetime(value: &str) -> Result<NaiveDateTime, RdbcError> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S%.f"))
        .or_else(|_| DateTime::parse_from_rfc3339(value).map(|value| value.naive_local()))
        .map_err(|error| RdbcError::DriverError(error.to_string()))
}

/// RBDC canonical 连接适配器。
///
/// 对应 Java 平台依赖: `java.sql.Connection` 的驱动实现。
/// 本对象包装单个 `rbdc::db::Connection`，不包装 RBDC Pool。
pub struct RbdcConnectionAdapter {
    connection: Option<Box<dyn rbdc::db::Connection>>,
    driver_name: String,
    closed: bool,
    discarded: bool,
    auto_commit: bool,
    savepoint_sequence: u64,
    standard_executor: Option<Arc<dyn StandardExecutor>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl RbdcConnectionAdapter {
    /// 包装一个未池化的 RBDC 连接。
    ///
    /// 参数 `connection` 为 RBDC 连接，`driver_name` 为驱动名称。
    pub fn new(connection: Box<dyn rbdc::db::Connection>, driver_name: impl Into<String>) -> Self {
        Self {
            connection: Some(connection),
            driver_name: driver_name.into(),
            closed: false,
            discarded: false,
            auto_commit: true,
            savepoint_sequence: 0,
            standard_executor: None,
            runtime_trace: None,
        }
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    fn connection_mut(&mut self) -> Result<&mut Box<dyn rbdc::db::Connection>, RdbcError> {
        if self.closed || self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.connection
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    fn standard_executor_core(&mut self) -> Result<Arc<dyn StandardExecutor>, RdbcError> {
        if let Some(executor) = &self.standard_executor {
            return Ok(Arc::clone(executor));
        }
        self.connection_mut()?;
        let runtime_trace = self.runtime_trace.take();
        let shell = Self {
            connection: None,
            driver_name: self.driver_name.clone(),
            closed: false,
            discarded: self.discarded,
            auto_commit: self.auto_commit,
            savepoint_sequence: self.savepoint_sequence,
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
        let result = self
            .connection_mut()?
            .exec(sql, Self::params_to_rbdc(params))
            .await
            .map_err(Self::driver_error)?;
        let last_insert_id = match Self::from_rbdc_value(result.last_insert_id)? {
            Value::Null => None,
            Value::Int(value) => Some(value),
            other => {
                return Err(RdbcError::DriverError(format!(
                    "RBDC last_insert_id must be an integer or null, got {other}"
                )));
            }
        };
        Ok(AdapterUpdateResult {
            rows_affected: result.rows_affected,
            last_insert_id,
            row_count: None,
        })
    }

    async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
        if let Some(mut connection) = self.connection.take() {
            connection.close().await.map_err(Self::driver_error)?;
        }
        self.closed = true;
        Ok(())
    }

    fn driver_error(error: rbdc::Error) -> RdbcError {
        // RBDC 4.9 的公开 `rbdc::Error` 是 `rbs::Error::E(String)`，没有
        // vendor code 或 SQLState。仍须保留“数据库驱动异常”这一结构化边界，
        // 让 ExceptionSorter 可以按消息规则分类，而不是降级成普通错误。
        RdbcError::SqlException(Box::new(
            SqlException::driver(0, error.to_string()).with_class_name("rbdc::Error"),
        ))
    }

    async fn fetch_rows_with_labels(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<(Vec<Row>, Vec<String>), RdbcError> {
        let mut stream = self
            .connection_mut()?
            .exec_rows(sql, Self::params_to_rbdc(params))
            .await
            .map_err(Self::driver_error)?;
        let mut rows = Vec::new();
        let mut labels = Vec::new();
        while let Some(row) = stream.next().await {
            let mut row = row.map_err(Self::driver_error)?;
            let meta_data = row.meta_data();
            let column_count = meta_data.column_len();
            if labels.is_empty() {
                labels = (0..column_count)
                    .map(|index| meta_data.column_name(index))
                    .collect();
            }
            let mut values = Vec::with_capacity(column_count);
            for index in 0..column_count {
                let value = row.get(index).map_err(Self::driver_error)?;
                values.push(Self::from_rbdc_value(value)?);
            }
            rows.push(Row::new(values));
        }
        Ok((rows, labels))
    }

    fn to_rbdc_value(value: Value) -> rbs::Value {
        match value {
            Value::Null => rbs::Value::Null,
            Value::Bool(value) => rbs::Value::Bool(value),
            Value::Int(value) => rbs::Value::I64(value),
            Value::Float(value) => rbs::Value::F64(value),
            Value::Decimal(value) => {
                rbs::Value::Ext("Decimal", Box::new(rbs::Value::String(value.to_string())))
            }
            Value::Date(value) => {
                rbs::Value::Ext("Date", Box::new(rbs::Value::String(value.to_string())))
            }
            Value::Time(value) => {
                rbs::Value::Ext("Time", Box::new(rbs::Value::String(value.to_string())))
            }
            // JDBC Timestamp 是不含时区的数据库日期时间；RBDC 对应 DateTime，
            // 而 RBDC Timestamp 是 Unix 毫秒瞬时值，不能混用。
            Value::Timestamp(value) => {
                rbs::Value::Ext("DateTime", Box::new(rbs::Value::String(value.to_string())))
            }
            Value::String(value) => rbs::Value::String(value),
            Value::Bytes(value) => rbs::Value::Binary(value),
        }
    }

    fn from_rbdc_value(value: rbs::Value) -> Result<Value, RdbcError> {
        match value {
            rbs::Value::Null => Ok(Value::Null),
            rbs::Value::Bool(value) => Ok(Value::Bool(value)),
            rbs::Value::I32(value) => Ok(Value::Int(i64::from(value))),
            rbs::Value::I64(value) => Ok(Value::Int(value)),
            rbs::Value::U32(value) => Ok(Value::Int(i64::from(value))),
            rbs::Value::U64(value) => i64::try_from(value)
                .map(Value::Int)
                .map_err(|_| RdbcError::DriverError("RBDC u64 value exceeds i64".to_string())),
            rbs::Value::F32(value) => Ok(Value::Float(f64::from(value))),
            rbs::Value::F64(value) => Ok(Value::Float(value)),
            rbs::Value::String(value) => Ok(Value::String(value)),
            rbs::Value::Binary(value) => Ok(Value::Bytes(value)),
            rbs::Value::Array(_) => Err(RdbcError::DriverError(
                "RBDC array value is not represented by druid::core::Value".to_string(),
            )),
            rbs::Value::Map(_) => Err(RdbcError::DriverError(
                "RBDC map value is not represented by druid::core::Value".to_string(),
            )),
            rbs::Value::Ext("Decimal", value) => parse_rbdc_string(*value, "Decimal")?
                .parse::<BigDecimal>()
                .map(Value::Decimal)
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            rbs::Value::Ext("Date", value) => {
                NaiveDate::parse_from_str(&parse_rbdc_string(*value, "Date")?, "%Y-%m-%d")
                    .map(Value::Date)
                    .map_err(|error| RdbcError::DriverError(error.to_string()))
            }
            rbs::Value::Ext("Time", value) => {
                NaiveTime::parse_from_str(&parse_rbdc_string(*value, "Time")?, "%H:%M:%S%.f")
                    .map(Value::Time)
                    .map_err(|error| RdbcError::DriverError(error.to_string()))
            }
            rbs::Value::Ext("DateTime", value) => {
                parse_rbdc_datetime(&parse_rbdc_string(*value, "DateTime")?).map(Value::Timestamp)
            }
            rbs::Value::Ext("Timestamp", value) => {
                let millis = value.as_i64().ok_or_else(|| {
                    RdbcError::DriverError(
                        "RBDC Timestamp extension must contain integer milliseconds".to_string(),
                    )
                })?;
                DateTime::<Utc>::from_timestamp_millis(millis)
                    .map(|value| Value::Timestamp(value.naive_utc()))
                    .ok_or_else(|| {
                        RdbcError::DriverError(format!(
                            "RBDC Timestamp milliseconds {millis} are out of range"
                        ))
                    })
            }
            rbs::Value::Ext(name, _) => Err(RdbcError::DriverError(format!(
                "RBDC extension value {name} is not represented by druid::core::Value"
            ))),
        }
    }

    fn params_to_rbdc(params: Vec<Value>) -> Vec<rbs::Value> {
        params.into_iter().map(Self::to_rbdc_value).collect()
    }
}

#[async_trait::async_trait]
impl AdapterExecutionCore for RbdcConnectionAdapter {
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
        self.fetch_rows_with_labels(sql, values)
            .await
            .map(|(rows, labels)| StandardQueryResult::with_labels(rows, labels))
    }

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.close_connection_core().await
    }
}

druid_rdbc::impl_standard_wrapper!(RbdcConnectionAdapter => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for RbdcConnectionAdapter {
    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Ok(Box::new(StandardAdapterStatement::new(
            self.standard_executor_core()?,
            options,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let parameter_count = executor.parameter_count(sql).await?;
        Ok(Box::new(StandardAdapterPreparedStatement::new(
            sql.to_owned(),
            executor,
            parameter_count,
            options,
        )))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        let mut snapshot =
            super::rbdc_database_meta_data::RbdcDatabaseMetaData::owned(self.driver_name.clone());
        let product_name =
            druid_rdbc::metadata::DatabaseMetaData::database_product_name(&mut snapshot)
                .await
                .ok();
        Ok(Box::new(StandardAdapterDatabaseMetaData::new(
            self.standard_executor_core()?,
            product_name,
            "rbdc_database_metadata_product_name",
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
            || self.closed || self.connection.is_none(),
            |executor| executor.is_closed(),
        ))
    }
}
