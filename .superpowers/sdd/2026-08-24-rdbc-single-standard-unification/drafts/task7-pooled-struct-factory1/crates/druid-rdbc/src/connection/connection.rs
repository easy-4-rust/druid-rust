use crate::error::RdbcError;
use crate::error::SqlWarning;
use crate::extensions::ConnectionCapabilities;
use crate::metadata::DatabaseMetaData;
use crate::statement::{
    CallableStatement, ExecResult, GeneratedKeys, ParameterIndex, PreparedInputParameter,
    PreparedParameter, PreparedStatement, ResultSetHoldability, Statement, StatementOptions,
};
use crate::types::RdbcTypeMap;
use crate::resource::{Array, Blob, Clob, NClob, SqlXml};
use crate::types::{RdbcObject, Row, Savepoint, Value};
use crate::wrapper::Wrapper;
use std::time::Duration;

/// JDBC transaction isolation level。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransactionIsolation(i32);

impl TransactionIsolation {
    /// 不支持 transaction。
    pub const NONE: Self = Self(0);
    /// 允许读取未提交数据。
    pub const READ_UNCOMMITTED: Self = Self(1);
    /// 只读取已提交数据。
    pub const READ_COMMITTED: Self = Self(2);
    /// Repeatable-read isolation。
    pub const REPEATABLE_READ: Self = Self(4);
    /// Serializable isolation。
    pub const SERIALIZABLE: Self = Self(8);

    /// 从 JDBC numeric code 创建 isolation level。
    pub const fn from_jdbc(code: i32) -> Self {
        Self(code)
    }

    /// 返回 JDBC numeric code。
    pub const fn jdbc_code(self) -> i32 {
        self.0
    }
}

/// JDBC `Connection` 的唯一 standard trait。
///
/// 对应 Java：`java.sql.Connection`。所有 mutable operations 使用 exclusive `&mut self`；
/// 创建的 statement 与 live metadata 拥有独立 actor/native handle，不借用 caller。
#[async_trait::async_trait]
pub trait Connection: Wrapper + Send + 'static {
    /// 按参数type_name与有序attributes创建独立拥有的SQL结构对象，返回对象或驱动错误。
    /// 对应 Java: Connection#createStruct；None保留原nullable输入供驱动验证。
    /// 类型名为NULL、数据库错误或连接已关闭时返回SQLException；不支持该类型时明确报错。
    async fn create_struct(
        &mut self,
        _type_name: Option<&str>,
        _attributes: Option<Vec<crate::types::RdbcObject>>,
    ) -> Result<Box<dyn crate::resource::Struct>, RdbcError> {
        Err(RdbcError::feature_not_supported("Connection#create_struct"))
    }

    /// 创建 owned statement。
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_create_statement",
        })
    }

    /// 在当前连接上执行 update 类 SQL。
    ///
    /// 这是从旧 driver SPI 合并来的 Rust extension；JDBC consumer 通常通过
    /// [`Statement::execute_update`] 调用同一行为。
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_exec",
        })
    }

    /// 在当前连接上执行 query 并返回 eager row values。
    ///
    /// 需要 owned cursor 的调用方应使用 [`Statement::execute_query`]。
    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_fetch",
        })
    }

    /// 顺序执行 driver-level update batch，并保留 64-bit counts 和 partial failure。
    async fn exec_batch(
        &mut self,
        batch: Vec<(String, Vec<Value>)>,
    ) -> Result<Vec<i64>, RdbcError> {
        let mut counts = Vec::with_capacity(batch.len());
        for (sql, params) in batch {
            match self.exec(&sql, params).await {
                Ok(result) => counts.push(i64::try_from(result.rows_affected).map_err(|_| {
                    RdbcError::InvalidArgument(
                        "batch update count exceeds canonical i64".to_owned(),
                    )
                })?),
                Err(cause) => {
                    return Err(RdbcError::BatchUpdateException {
                        update_counts: counts.into(),
                        cause: Box::new(cause),
                    });
                }
            }
        }
        Ok(counts)
    }

    /// 通过 canonical PreparedStatement 执行一组 value snapshots。
    ///
    /// 本 convenience 保留旧 connection-owned batch 调用路径，但 statement 自身仍拥有
    /// parameter/batch state；adapter 只需实现 [`PreparedStatement`] 一次。
    async fn exec_prepared_batch(
        &mut self,
        statement: &mut dyn PreparedStatement,
        parameter_sets: Vec<Vec<Value>>,
    ) -> Result<Vec<i64>, RdbcError> {
        statement.clear_prepared_batch()?;
        for values in parameter_sets {
            let parameters = values
                .into_iter()
                .enumerate()
                .map(|(offset, value)| {
                    Ok(PreparedParameter {
                        index: ParameterIndex::new(offset + 1)?,
                        value,
                    })
                })
                .collect::<Result<Vec<_>, RdbcError>>()?;
            statement.add_parameter_batch(parameters)?;
        }
        statement.execute_prepared_batch().await
    }

    /// 执行 rich prepared-parameter batch，不提前物化 LOB/stream descriptors。
    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &mut dyn PreparedStatement,
        parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i64>, RdbcError> {
        statement.clear_prepared_batch()?;
        for parameters in parameter_sets {
            let parameters = parameters
                .into_iter()
                .enumerate()
                .map(|(offset, parameter)| Ok((ParameterIndex::new(offset + 1)?, parameter)))
                .collect::<Result<Vec<_>, RdbcError>>()?;
            statement.add_input_parameter_batch(parameters)?;
        }
        statement.execute_prepared_batch().await
    }

    /// 创建 owned prepared statement。
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_prepare_statement",
        })
    }

    /// 按类型化生成键请求创建拥有独立句柄的预编译语句。
    ///
    /// 对应 Java: `Connection#prepareStatement(String, int/int[]/String[])`。
    /// `sql` 为待准备 SQL，`options` 为结果集选项，`generated_keys` 保留请求模式及列顺序。
    /// 返回 owned 预编译语句；默认仅将 `None` 委托普通准备入口，其他请求在创建句柄前
    /// 明确报不支持，避免静默丢失请求或遗留已创建的底层资源。
    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: StatementOptions,
        generated_keys: GeneratedKeys,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        match generated_keys {
            GeneratedKeys::None => self.prepare_statement(sql, options).await,
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "connection_prepare_statement_generated_keys",
            }),
        }
    }

    /// 创建 owned callable statement。
    async fn prepare_call(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_prepare_call",
        })
    }

    /// 返回 owned live database metadata object。
    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_database_meta_data",
        })
    }

    /// 返回 database product name；SQL NULL 保持 `None`。
    async fn database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
        let mut metadata = self.database_meta_data().await?;
        metadata
            .metadata_string(crate::metadata::DatabaseMetaDataRequest::new(
                crate::metadata::DatabaseMetaDataOperation::GetDatabaseProductName,
                Vec::new(),
            ))
            .await
    }

    /// 返回 database product version；SQL NULL 保持 `None`。
    async fn database_product_version(&mut self) -> Result<Option<String>, RdbcError> {
        let mut metadata = self.database_meta_data().await?;
        metadata
            .metadata_string(crate::metadata::DatabaseMetaDataRequest::new(
                crate::metadata::DatabaseMetaDataOperation::GetDatabaseProductVersion,
                Vec::new(),
            ))
            .await
    }

    /// 返回 driver major version。
    fn driver_major_version(&self) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_driver_major_version",
        })
    }

    /// 返回 driver minor version。
    fn driver_minor_version(&self) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_driver_minor_version",
        })
    }

    /// 提交当前 transaction。
    async fn commit(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_commit",
        })
    }

    /// 回滚当前 transaction。
    async fn rollback(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_rollback",
        })
    }

    /// 回滚到保存点。
    async fn rollback_to(&mut self, _savepoint: &Savepoint) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_rollback_to",
        })
    }

    /// 开始 transaction；默认通过关闭 auto-commit 表达。
    async fn begin(&mut self) -> Result<(), RdbcError> {
        self.set_auto_commit(false).await
    }

    /// 创建匿名保存点。
    async fn set_savepoint(&mut self) -> Result<Savepoint, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_savepoint",
        })
    }

    /// 创建命名保存点。
    async fn set_savepoint_named(&mut self, _name: &str) -> Result<Savepoint, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_savepoint_named",
        })
    }

    /// 释放保存点。
    async fn release_savepoint(&mut self, _savepoint: &Savepoint) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_release_savepoint",
        })
    }

    /// 设置 auto-commit 状态。
    async fn set_auto_commit(&mut self, _enabled: bool) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_auto_commit",
        })
    }

    /// 返回 auto-commit 状态。
    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_auto_commit",
        })
    }

    /// 返回当前只读状态。
    fn read_only(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_read_only",
        })
    }

    /// 设置当前只读状态。
    async fn set_read_only(&mut self, _read_only: bool) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_read_only",
        })
    }

    /// 设置一个 client-info property；`None` 表示删除。
    async fn set_client_info(
        &mut self,
        _name: &str,
        _value: Option<&str>,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_client_info",
        })
    }

    /// 返回一个 client-info property。
    fn client_info(&self, _name: &str) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_client_info",
        })
    }

    /// 把 SQL 转成 driver-native SQL；默认 identity，禁止猜测重写。
    async fn native_sql(&mut self, sql: &str) -> Result<String, RdbcError> {
        Ok(sql.to_owned())
    }

    /// 设置 network timeout；未支持的 adapter fail closed。
    async fn set_network_timeout(&mut self, _timeout: Duration) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_network_timeout",
        })
    }

    /// 返回 network timeout；`Duration::ZERO` 表示未设置。
    fn network_timeout(&self) -> Result<Duration, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_network_timeout",
        })
    }

    /// 返回 typed SQL user-defined type map。
    fn type_map(&self) -> Result<Option<RdbcTypeMap>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_type_map",
        })
    }

    /// 设置 typed SQL user-defined type map。
    async fn set_type_map(&mut self, _type_map: RdbcTypeMap) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_type_map",
        })
    }

    /// 返回 transaction isolation level。
    fn transaction_isolation(&self) -> Result<TransactionIsolation, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_transaction_isolation",
        })
    }

    /// 设置 transaction isolation level。
    async fn set_transaction_isolation(
        &mut self,
        _level: TransactionIsolation,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_transaction_isolation",
        })
    }

    /// 返回 ResultSet holdability。
    fn holdability(&self) -> Result<ResultSetHoldability, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_holdability",
        })
    }

    /// 设置 ResultSet holdability。
    async fn set_holdability(
        &mut self,
        _holdability: ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_holdability",
        })
    }

    /// 返回当前 catalog；`None` 保留 JDBC `null` 语义。
    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_catalog",
        })
    }

    /// 设置当前 catalog；`None` 保留 JDBC `null` 语义。
    async fn set_catalog(&mut self, _catalog: Option<&str>) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_catalog",
        })
    }

    /// 返回当前 schema；`None` 保留 JDBC `null` 语义。
    fn schema(&self) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_schema",
        })
    }

    /// 设置当前 schema；`None` 保留 JDBC `null` 语义。
    async fn set_schema(&mut self, _schema: Option<&str>) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_schema",
        })
    }

    /// 返回当前 JDBC warning 链。
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_warnings",
        })
    }

    /// 清理当前 JDBC warning 链。
    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_clear_warnings",
        })
    }

    /// 创建独占的标准 Blob 资源。对应 Java: `Connection#createBlob`。
    ///
    /// 无参数；返回不借用调用方的 `Send + 'static` 资源对象，驱动不支持时返回错误。
    async fn create_blob(&mut self) -> Result<Box<dyn Blob>, RdbcError> {
        Err(RdbcError::feature_not_supported("connection_create_blob"))
    }

    /// 创建独占的标准 Clob 资源。对应 Java: `Connection#createClob`。
    ///
    /// 无参数；返回不借用调用方的 `Send + 'static` 资源对象，驱动不支持时返回错误。
    async fn create_clob(&mut self) -> Result<Box<dyn Clob>, RdbcError> {
        Err(RdbcError::feature_not_supported("connection_create_clob"))
    }

    /// 创建独占的标准 NClob 资源。对应 Java: `Connection#createNClob`。
    ///
    /// 无参数；返回不借用调用方的 `Send + 'static` 资源对象，驱动不支持时返回错误。
    async fn create_n_clob(&mut self) -> Result<Box<dyn NClob>, RdbcError> {
        Err(RdbcError::feature_not_supported("connection_create_n_clob"))
    }

    /// 创建初始不含数据的独占 SQLXML 资源。对应 Java: `Connection#createSQLXML`。
    ///
    /// 无参数；返回不借用调用方的标准资源对象，后续可通过 SQLXML 写入方法填充数据。
    /// 驱动不支持该 SQL 类型时返回标准不支持错误；关闭连接由驱动拒绝创建。
    async fn create_sql_xml(&mut self) -> Result<Box<dyn SqlXml>, RdbcError> {
        Err(RdbcError::feature_not_supported("connection_create_sql_xml"))
    }

    /// 创建 SQL 元素数组。对应 Java: `Connection#createArrayOf(String, Object[])`。
    ///
    /// `type_name` 是数据库支持的内置、用户定义或标准 SQL 元素类型名称；
    /// `None` 保留 Java null 类型名，由驱动返回错误，不能替换为空字符串。
    /// `elements` 保留 null 数组、空数组及元素顺序，元素的 SQL NULL 使用 `RdbcObject` 表示。
    /// 驱动负责 JDBC 默认元素类型映射；不兼容转换及 primitive/object 数组表示由驱动决定。
    /// 返回独立拥有的标准 Array，`base_type_name` 应与请求类型对应；不支持时明确返回错误。
    async fn create_array_of(
        &mut self,
        _type_name: Option<&str>,
        _elements: Option<Vec<RdbcObject>>,
    ) -> Result<Box<dyn Array>, RdbcError> {
        Err(RdbcError::feature_not_supported("connection_create_array_of"))
    }

    /// 返回 immutable capability descriptor。
    fn capabilities(&self) -> ConnectionCapabilities {
        ConnectionCapabilities::default()
    }

    /// 以 bounded timeout 验证连接。
    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_is_valid",
        })
    }

    /// 验证连接；失败时保留 typed driver error。
    async fn ping(&mut self) -> Result<(), RdbcError> {
        if self.is_valid(0).await? {
            Ok(())
        } else {
            Err(RdbcError::ConnectionDiscarded)
        }
    }

    /// 强制中止连接；默认复用幂等 close。
    async fn abort(&mut self) -> Result<(), RdbcError> {
        self.close().await
    }

    /// 关闭 raw 或 logical connection handle。
    async fn close(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::feature_not_supported("connection_close"))
    }

    /// 返回 connection 是否已经关闭。
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::feature_not_supported("connection_is_closed"))
    }

    /// 返回 driver identity；不支持时 fail closed。
    fn driver_name(&self) -> Result<String, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_driver_name",
        })
    }
}
