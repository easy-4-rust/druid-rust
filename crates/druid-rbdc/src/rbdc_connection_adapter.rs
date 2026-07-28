//! RBDC 物理连接适配器。

use druid_core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalPreparedStatement, PreparedStatementKey, Row, Savepoint, SqlTextPreparedStatement,
    Value,
};
use futures::StreamExt;
use std::sync::Arc;

/// RBDC 物理连接适配器。
///
/// 对应 Java 平台依赖: `java.sql.Connection` 的驱动实现。
/// 本对象包装单个 `rbdc::db::Connection`，不包装 RBDC Pool。
pub struct RbdcConnectionAdapter {
    connection: Box<dyn rbdc::db::Connection>,
    driver_name: String,
    closed: bool,
    discarded: bool,
    auto_commit: bool,
    savepoint_sequence: u64,
}

impl RbdcConnectionAdapter {
    /// 包装一个未池化的 RBDC 连接。
    ///
    /// 参数 `connection` 为 RBDC 连接，`driver_name` 为驱动名称。
    pub fn new(connection: Box<dyn rbdc::db::Connection>, driver_name: impl Into<String>) -> Self {
        Self {
            connection,
            driver_name: driver_name.into(),
            closed: false,
            discarded: false,
            auto_commit: true,
            savepoint_sequence: 0,
        }
    }

    fn driver_error(error: rbdc::Error) -> DruidError {
        DruidError::DriverError(error.to_string())
    }

    fn to_rbdc_value(value: Value) -> rbs::Value {
        match value {
            Value::Null => rbs::Value::Null,
            Value::Bool(value) => rbs::Value::Bool(value),
            Value::Int(value) => rbs::Value::I64(value),
            Value::Float(value) => rbs::Value::F64(value),
            Value::String(value) => rbs::Value::String(value),
            Value::Bytes(value) => rbs::Value::Binary(value),
        }
    }

    fn from_rbdc_value(value: rbs::Value) -> Result<Value, DruidError> {
        match value {
            rbs::Value::Null => Ok(Value::Null),
            rbs::Value::Bool(value) => Ok(Value::Bool(value)),
            rbs::Value::I32(value) => Ok(Value::Int(i64::from(value))),
            rbs::Value::I64(value) => Ok(Value::Int(value)),
            rbs::Value::U32(value) => Ok(Value::Int(i64::from(value))),
            rbs::Value::U64(value) => i64::try_from(value)
                .map(Value::Int)
                .map_err(|_| DruidError::DriverError("RBDC u64 value exceeds i64".to_string())),
            rbs::Value::F32(value) => Ok(Value::Float(f64::from(value))),
            rbs::Value::F64(value) => Ok(Value::Float(value)),
            rbs::Value::String(value) => Ok(Value::String(value)),
            rbs::Value::Binary(value) => Ok(Value::Bytes(value)),
            rbs::Value::Array(_) => Err(DruidError::DriverError(
                "RBDC array value is not represented by druid_core::Value".to_string(),
            )),
            rbs::Value::Map(_) => Err(DruidError::DriverError(
                "RBDC map value is not represented by druid_core::Value".to_string(),
            )),
            rbs::Value::Ext(name, _) => Err(DruidError::DriverError(format!(
                "RBDC extension value {name} is not represented by druid_core::Value"
            ))),
        }
    }

    fn params_to_rbdc(params: Vec<Value>) -> Vec<rbs::Value> {
        params.into_iter().map(Self::to_rbdc_value).collect()
    }

    fn validate_savepoint_name(name: &str) -> Result<(), DruidError> {
        let valid = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(())
        } else {
            Err(DruidError::DriverError(
                "savepoint name must contain only ASCII letters, digits, or '_'".to_string(),
            ))
        }
    }

    async fn execute_control_statement(&mut self, sql: &str) -> Result<(), DruidError> {
        self.connection
            .exec(sql, Vec::new())
            .await
            .map_err(Self::driver_error)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for RbdcConnectionAdapter {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        let result = self
            .connection
            .exec(sql, Self::params_to_rbdc(params))
            .await
            .map_err(Self::driver_error)?;
        let last_insert_id = match Self::from_rbdc_value(result.last_insert_id)? {
            Value::Null => None,
            Value::Int(value) => Some(value),
            other => {
                return Err(DruidError::DriverError(format!(
                    "RBDC last_insert_id must be an integer or null, got {other}"
                )));
            }
        };
        Ok(ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id,
            row_count: None,
        })
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        let mut stream = self
            .connection
            .exec_rows(sql, Self::params_to_rbdc(params))
            .await
            .map_err(Self::driver_error)?;
        let mut rows = Vec::new();
        while let Some(row) = stream.next().await {
            let mut row = row.map_err(Self::driver_error)?;
            let column_count = row.meta_data().column_len();
            let mut values = Vec::with_capacity(column_count);
            for index in 0..column_count {
                let value = row.get(index).map_err(Self::driver_error)?;
                values.push(Self::from_rbdc_value(value)?);
            }
            rows.push(Row::new(values));
        }
        Ok(rows)
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        if self.closed || self.discarded {
            return Err(DruidError::ConnectionDiscarded);
        }
        // RBDC 的公开 Connection SPI 在 exec 内部完成驱动 prepare/cache，
        // 因此这里保存经过完整 Druid key 区分后的 SQL token。
        Ok(Arc::new(SqlTextPreparedStatement::new(key.sql())))
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        self.connection.begin().await.map_err(Self::driver_error)?;
        self.auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        self.connection.commit().await.map_err(Self::driver_error)?;
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        self.connection
            .rollback()
            .await
            .map_err(Self::driver_error)?;
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        Self::validate_savepoint_name(&name)?;
        self.execute_control_statement(&format!("ROLLBACK TO SAVEPOINT {name}"))
            .await
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        let savepoint = Savepoint {
            id: self.savepoint_sequence,
            name: None,
        };
        self.execute_control_statement(&format!("SAVEPOINT druid_sp_{}", savepoint.id))
            .await?;
        Ok(savepoint)
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, DruidError> {
        Self::validate_savepoint_name(name)?;
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        self.execute_control_statement(&format!("SAVEPOINT {name}"))
            .await?;
        Ok(Savepoint {
            id: self.savepoint_sequence,
            name: Some(name.to_string()),
        })
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        Self::validate_savepoint_name(&name)?;
        self.execute_control_statement(&format!("RELEASE SAVEPOINT {name}"))
            .await
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        self.connection.ping().await.map_err(Self::driver_error)
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        if !self.closed {
            self.connection.close().await.map_err(Self::driver_error)?;
            self.closed = true;
        }
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        PhysicalConnectionCapabilities {
            transactions: true,
            savepoints: true,
            auto_commit: true,
            read_only: false,
            transaction_isolation: false,
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

    fn mark_discarded(&mut self) {
        self.discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.discarded
    }

    fn driver_name(&self) -> &str {
        &self.driver_name
    }
}
