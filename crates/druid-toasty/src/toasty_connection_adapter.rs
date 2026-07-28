//! Toasty 物理连接适配器。

use druid_core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalPreparedStatement, PreparedStatementKey, Row, Savepoint, SqlTextPreparedStatement,
    Value,
};
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

/// 将一个未池化 Toasty driver connection 适配为 Druid 物理连接。
///
/// 对应 Java 平台对象：JDBC driver 的 `java.sql.Connection` 实现。对象不持有
/// `toasty::Db` 或 Toasty Pool；DruidPool 独占池化、回收和统计职责。
pub struct ToastyConnectionAdapter {
    connection: Option<Box<dyn ToastyConnection>>,
    schema: Arc<Schema>,
    driver_name: &'static str,
    auto_commit: bool,
    read_only: bool,
    isolation: Option<IsolationLevel>,
    savepoint_sequence: u64,
    discarded: bool,
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
    ) -> Self {
        Self {
            connection: Some(connection),
            schema,
            driver_name: capability.driver_name,
            auto_commit: true,
            read_only: false,
            isolation: None,
            savepoint_sequence: 0,
            discarded: false,
        }
    }

    fn connection_mut(&mut self) -> Result<&mut Box<dyn ToastyConnection>, DruidError> {
        if self.discarded {
            return Err(DruidError::ConnectionDiscarded);
        }
        self.connection
            .as_mut()
            .ok_or(DruidError::ConnectionDiscarded)
    }

    fn driver_error(error: &toasty_core::Error) -> DruidError {
        if error.is_connection_lost() {
            DruidError::ConnectionDiscarded
        } else {
            DruidError::DriverError(error.to_string())
        }
    }

    async fn execute_operation(
        &mut self,
        operation: Operation,
    ) -> Result<toasty_core::driver::ExecResponse, DruidError> {
        let schema = Arc::clone(&self.schema);
        self.connection_mut()?
            .exec(&schema, operation)
            .await
            .map_err(|error| Self::driver_error(&error))
    }

    fn typed_parameter(value: Value) -> TypedValue {
        match value {
            // Druid Value 目前不携带 JDBC targetSqlType；与现有 SQLx/RBDC
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
            Value::String(value) => TypedValue {
                value: ToastyValue::String(value),
                ty: db::Type::Text,
            },
            Value::Bytes(value) => TypedValue {
                value: ToastyValue::Bytes(value),
                ty: db::Type::Blob,
            },
        }
    }

    fn raw_sql(sql: &str, params: Vec<Value>, ret: RawSqlRet) -> Operation {
        RawSql {
            sql: sql.to_string(),
            params: params.into_iter().map(Self::typed_parameter).collect(),
            ret,
        }
        .into()
    }

    fn value(value: ToastyValue) -> Result<Value, DruidError> {
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
                DruidError::DriverError(
                    "Toasty u64 result cannot be represented by druid_core::Value::Int".to_string(),
                )
            }),
            ToastyValue::F32(value) => Ok(Value::Float(f64::from(value))),
            ToastyValue::F64(value) => Ok(Value::Float(value)),
            ToastyValue::String(value) => Ok(Value::String(value)),
            ToastyValue::Bytes(value) => Ok(Value::Bytes(value)),
            unsupported => Err(DruidError::DriverError(format!(
                "Toasty value {unsupported:?} is not represented by druid_core::Value"
            ))),
        }
    }

    fn row(value: ToastyValue) -> Result<Row, DruidError> {
        let ToastyValue::Record(record) = value else {
            return Err(DruidError::DriverError(format!(
                "Toasty raw SQL row must be Value::Record, actual={value:?}"
            )));
        };
        record
            .into_iter()
            .map(Self::value)
            .collect::<Result<Vec<_>, _>>()
            .map(Row::new)
    }

    fn validate_savepoint_name(name: &str) -> Result<(), DruidError> {
        let valid = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(())
        } else {
            Err(DruidError::InvalidArgument(
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

    fn ensure_transaction(&self) -> Result<(), DruidError> {
        if self.auto_commit {
            Err(DruidError::DriverError(
                "transaction operation requires auto_commit=false".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    async fn sqlite_last_insert_id(&mut self) -> Result<Option<i64>, DruidError> {
        if !self.is_sqlite() {
            return Ok(None);
        }
        let response = self
            .execute_operation(Self::raw_sql(
                "SELECT last_insert_rowid()",
                Vec::new(),
                RawSqlRet::Infer,
            ))
            .await?;
        let values = response
            .values
            .collect_as_value()
            .await
            .map_err(|error| Self::driver_error(&error))?;
        let ToastyValue::List(mut rows) = values else {
            return Err(DruidError::DriverError(format!(
                "SQLite last_insert_rowid() must return a row list, actual={values:?}"
            )));
        };
        let Some(ToastyValue::Record(mut row)) = rows.pop() else {
            return Ok(None);
        };
        match row.fields.pop() {
            Some(ToastyValue::I64(0)) | None => Ok(None),
            Some(ToastyValue::I64(value)) => Ok(Some(value)),
            value => Err(DruidError::DriverError(format!(
                "SQLite last_insert_rowid() must return i64, actual={value:?}"
            ))),
        }
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for ToastyConnectionAdapter {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        let response = self
            .execute_operation(Self::raw_sql(sql, params, RawSqlRet::None))
            .await?;
        let Rows::Count(rows_affected) = response.values else {
            return Err(DruidError::DriverError(
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

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        let response = self
            .execute_operation(Self::raw_sql(sql, params, RawSqlRet::Infer))
            .await?;
        let values = response
            .values
            .collect_as_value()
            .await
            .map_err(|error| Self::driver_error(&error))?;
        let ToastyValue::List(rows) = values else {
            return Err(DruidError::DriverError(format!(
                "Toasty query must return a row list, actual={values:?}"
            )));
        };
        rows.into_iter().map(Self::row).collect()
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        let _ = self.connection_mut()?;
        // Toasty SQL drivers prepare/cache raw SQL inside Connection#exec。
        // Druid holder 保留逻辑句柄及缓存命中，执行仍走同一物理连接。
        Ok(Arc::new(SqlTextPreparedStatement::new(key.sql())))
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        if !self.auto_commit {
            return Err(DruidError::DriverError(
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

    async fn commit(&mut self) -> Result<(), DruidError> {
        self.ensure_transaction()?;
        self.execute_operation(Transaction::Commit.into()).await?;
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        self.ensure_transaction()?;
        self.execute_operation(Transaction::Rollback.into()).await?;
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.ensure_transaction()?;
        let name = Self::savepoint_name(savepoint);
        Self::validate_savepoint_name(&name)?;
        self.execute_operation(Transaction::RollbackToSavepoint(name).into())
            .await?;
        Ok(())
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
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

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, DruidError> {
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

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.ensure_transaction()?;
        let name = Self::savepoint_name(savepoint);
        Self::validate_savepoint_name(&name)?;
        self.execute_operation(Transaction::ReleaseSavepoint(name).into())
            .await?;
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        self.connection_mut()?
            .ping()
            .await
            .map_err(|error| Self::driver_error(&error))
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        // Toasty Connection SPI 以 Drop 关闭 raw driver connection。
        // SQLite/rusqlite 与网络驱动都会在 drop 时回滚未提交事务。
        self.connection.take();
        self.auto_commit = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.connection.is_none()
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        let sqlite = self.is_sqlite();
        PhysicalConnectionCapabilities {
            transactions: true,
            savepoints: true,
            auto_commit: true,
            read_only: !sqlite,
            transaction_isolation: true,
            holdability: false,
            clear_warnings: false,
            catalog: false,
            schema: false,
        }
    }

    fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        match (self.auto_commit, auto_commit) {
            (true, false) => self.begin().await,
            (false, true) => self.commit().await,
            _ => Ok(()),
        }
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), DruidError> {
        if !self.auto_commit {
            return Err(DruidError::InvalidArgument(
                "read_only cannot change while a transaction is active".to_string(),
            ));
        }
        if self.is_sqlite() && read_only {
            return Err(DruidError::UnsupportedOperation {
                operation: "toasty_sqlite_read_only_transaction",
            });
        }
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

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), DruidError> {
        if !self.auto_commit {
            return Err(DruidError::InvalidArgument(
                "transaction isolation cannot change while a transaction is active".to_string(),
            ));
        }
        if self.is_sqlite() && level != 8 {
            return Err(DruidError::InvalidArgument(
                "Toasty SQLite only supports JDBC TRANSACTION_SERIALIZABLE (8)".to_string(),
            ));
        }
        self.isolation = Some(match level {
            1 => IsolationLevel::ReadUncommitted,
            2 => IsolationLevel::ReadCommitted,
            4 => IsolationLevel::RepeatableRead,
            8 => IsolationLevel::Serializable,
            _ => {
                return Err(DruidError::InvalidArgument(format!(
                    "unsupported JDBC transaction isolation level: {level}"
                )));
            }
        });
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
