//! 对应 Java 类：com.alibaba.druid.filter.Filter
//! 来源文件：core/src/main/java/com/alibaba/druid/filter/Filter.java
//!
//! Filter trait 定义，对齐 DruidJava Filter 的核心 hook 方法。
//! DruidJava 有 100+ hook，这里保留最常用的 20+ 核心 hook，
//! 其余通过扩展 trait（ConnectionHook / StatementHook）覆盖。

use super::error::DruidError;
use super::value::Value;
use super::{
    ConnectionDatabaseMetaDataFilterChain, ConnectionWarningFilterChain,
    DataSourceGetConnectionFilterChain, DataSourceReleaseConnectionFilterChain,
    DruidPooledConnection, PhysicalConnectionCloseFilterChain,
    PhysicalConnectionConnectFilterChain, PhysicalConnectionConnectResult,
    PhysicalDatabaseMetaData, PreparedInputParameter, SqlWarning, StatementWarningFilterChain,
};
use std::time::{Duration, Instant};

/// 配置加载与旧 Druid 密文兼容。
pub mod config;
/// JDBC 字符编码转换 Filter。
pub mod encoding;
/// MySQL Connector/J 8 日期时间兼容 Filter。
pub mod mysql8datetime;

/// SQL 执行上下文，传递给 Filter 的 before/after 方法。
///
/// 对应 DruidJava Filter 方法的各种参数。
#[derive(Debug)]
pub struct ExecContext<'a> {
    /// 创建本次执行的 Druid 连接 ID。
    pub connection_id: u64,
    /// 创建本次执行的逻辑 Statement ID；直接通过连接 SPI 执行时为空。
    pub statement_id: Option<u64>,
    /// SQL 文本
    pub sql: String,
    /// SQL 参数
    pub params: &'a [Value],
    /// PreparedStatement setter 的完整参数描述符。
    ///
    /// 普通 Statement 为 `None`。PreparedStatement 执行时保留 setter 类型、
    /// nullable、长度及资源句柄；`params` 仅作为标量兼容视图，不能替代本字段。
    pub prepared_parameters: Option<&'a [PreparedInputParameter]>,
    /// 数据源名称
    pub data_source: &'a str,
    /// 执行开始时间
    pub start: Instant,
    /// SQL 指纹（参数化后的哈希）
    pub fingerprint: Option<u64>,
    /// 当前物理连接是否处于显式事务。
    pub in_transaction: bool,
    /// 本次 SQL 的 JDBC 执行入口语义。
    pub operation: ExecOperation,
}

/// JDBC 批处理 Filter 上下文。
///
/// 对应 Java `StatementProxy#getBatchSql()` 与 `getBatchSqlList()`。`sql` 使用
/// Java 的 `"\n;\n"` 连接规则，`statements` 保留每条 SQL 和批次大小。
#[derive(Debug)]
pub struct BatchExecContext<'a> {
    /// 创建本次批处理的 Druid 连接 ID。
    pub connection_id: u64,
    /// 创建本次批处理的逻辑 Statement ID；直接通过连接 SPI 执行时为空。
    pub statement_id: Option<u64>,
    /// 合并后的批处理 SQL。
    pub sql: &'a str,
    /// 按 `addBatch` 顺序保存的 SQL。
    pub statements: &'a [String],
    /// 按 `PreparedStatement#addBatch()` 顺序保存的参数快照。
    ///
    /// 普通 Statement 为空；PreparedStatement 的 Java 代理不把参数批次放入
    /// `getBatchSqlList()`，因此本字段不能用于 StatFilter 的 batch-size 统计。
    pub parameter_sets: &'a [Vec<Value>],
    /// PreparedStatement 参数描述符批次；普通 Statement 为 `None`。
    pub prepared_parameter_sets: Option<&'a [Vec<PreparedInputParameter>]>,
    /// 区分普通 Statement 与 PreparedStatement 的回调语义。
    pub kind: BatchExecKind,
    /// 数据源名称。
    pub data_source: &'a str,
    /// 批处理开始时间。
    pub start: Instant,
    /// SQL 统计对象的 key；由统计 Filter 在 before 阶段填充。
    pub fingerprint: Option<u64>,
    /// 当前物理连接是否处于显式事务。
    pub in_transaction: bool,
}

/// JDBC 批处理入口种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchExecKind {
    /// `Statement#executeBatch()`。
    Statement,
    /// `PreparedStatement#executeBatch()`。
    PreparedStatement,
}

/// SQL 执行入口类型。
///
/// 对应 Java `StatementExecuteType` 中本层已具备真实物理能力的查询与更新入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecOperation {
    /// `Statement#execute(...)` / `PreparedStatement#execute()` generic 入口。
    Execute,
    /// `executeQuery` / 查询类 PreparedStatement。
    Query,
    /// `executeUpdate` / 更新类 PreparedStatement。
    Update,
    /// `executeBatch` 批处理入口。
    Batch,
}

/// 连接事件类型，对应 DruidJava 的 connection_* hook 系列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// 连接创建（对应 connection_connect）
    Connect,
    /// 连接关闭（对应 connection_close）
    Close,
    /// 设置自动提交（对应 connection_setAutoCommit）
    SetAutoCommit(bool),
    /// 获取自动提交（对应 connection_getAutoCommit）
    GetAutoCommit,
    /// 提交事务（对应 connection_commit）
    Commit,
    /// 回滚事务（对应 connection_rollback）
    Rollback,
    /// 设置只读（对应 connection_setReadOnly）
    SetReadOnly(bool),
    /// 获取只读（对应 connection_isReadOnly）
    GetReadOnly,
    /// 设置 catalog（对应 connection_setCatalog）
    SetCatalog(String),
    /// 获取 catalog（对应 connection_getCatalog）
    GetCatalog,
    /// 设置事务隔离级别（对应 connection_setTransactionIsolation）
    SetTransactionIsolation(u8),
    /// 获取事务隔离级别（对应 connection_getTransactionIsolation）
    GetTransactionIsolation,
    /// 清除警告（对应 connection_clearWarnings）
    ClearWarnings,
    /// 设置 schema（对应 connection_setSchema）
    SetSchema(String),
    /// 获取 schema（对应 connection_getSchema）
    GetSchema,
    /// 中止连接（对应 connection_abort）
    Abort,
    /// 验证连接（对应 connection_isValid）
    IsValid,
    /// 原生 SQL（对应 connection_nativeSQL）
    NativeSQL(String),
    /// 设置网络超时（对应 connection_setNetworkTimeout）
    SetNetworkTimeout(Duration),
    /// 获取网络超时（对应 connection_getNetworkTimeout）
    GetNetworkTimeout,
}

/// 携带 Druid 连接身份的连接 Filter 事件。
#[derive(Debug, Clone, Copy)]
pub struct ConnectionEventContext<'a> {
    /// Druid 物理连接 ID。
    pub connection_id: u64,
    /// Java 连接事件。
    pub event: &'a ConnectionEvent,
}

/// 语句事件类型，对应 DruidJava 的 statement_* / preparedStatement_* hook 系列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementEvent {
    /// 创建 Statement（对应 connection_createStatement）
    CreateStatement,
    /// 创建 PreparedStatement（对应 connection_prepareStatement）
    PrepareStatement(String),
    /// 创建 CallableStatement（对应 connection_prepareCall）
    PrepareCall(String),
    /// 执行语句（对应 statement_execute）
    Execute(String),
    /// 执行查询（对应 statement_executeQuery）
    ExecuteQuery(String),
    /// 执行更新（对应 statement_executeUpdate）
    ExecuteUpdate(String),
    /// 关闭语句（对应 statement_close）
    Close,
    /// 批量执行（对应 statement_executeBatch）
    ExecuteBatch,
}

/// 携带 Connection/Statement Proxy 身份的语句事件。
#[derive(Debug, Clone, Copy)]
pub struct StatementEventContext<'a> {
    /// 创建语句的 Druid 连接 ID。
    pub connection_id: u64,
    /// 逻辑 Statement ID。
    pub statement_id: u64,
    /// Java Statement 事件。
    pub event: &'a StatementEvent,
}

/// 结果集事件类型，对应 DruidJava 的 resultSet_* hook 系列。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultSetEvent {
    /// 移到下一行（对应 resultSet_next）
    Next,
    /// 关闭结果集（对应 resultSet_close）
    Close,
    /// 获取 String（对应 resultSet_getString）
    GetString,
    /// 获取 Boolean（对应 resultSet_getBoolean）
    GetBoolean,
    /// 获取 Int（对应 resultSet_getInt）
    GetInt,
    /// 移到首行（对应 resultSet_first）
    First,
    /// 移到末行（对应 resultSet_last）
    Last,
}

/// 前置 Filter trait。
///
/// 对应 DruidJava `Filter` 接口的 before 系列 hook。
/// 在 SQL 执行前调用，任一 Filter 返回 Err 则短路。
#[async_trait::async_trait]
pub trait BeforeFilter: Send + Sync {
    /// 返回 Filter 名称。
    fn name(&self) -> &str;

    /// 包围一次数据源池化连接获取。
    ///
    /// 对应 Java：
    /// `Filter#dataSource_getConnection(FilterChain,DruidDataSource,long)`。
    /// 默认继续调用有位置链；实现可以修改 `max_wait`、短路返回自己的池化连接
    /// 或返回错误。该 hook 与物理 `connection_connect` 是两层独立调用。
    async fn data_source_get_connection(
        &self,
        chain: &mut DataSourceGetConnectionFilterChain<'_>,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        chain.data_source_get_connection(max_wait).await
    }

    /// 包围一次池化连接归还。
    ///
    /// 对应 Java：
    /// `Filter#dataSource_releaseConnection(FilterChain,DruidPooledConnection)`。
    /// 默认继续链；Filter 可以在末端回收前后执行逻辑、返回错误或有意短路。
    async fn data_source_release_connection(
        &self,
        chain: &mut DataSourceReleaseConnectionFilterChain<'_>,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        chain.data_source_recycle(connection).await
    }

    /// 包围一次真实物理连接创建。
    ///
    /// 对应 Java：
    /// `Filter#connection_connect(FilterChain, Properties)`。Filter 可以原地
    /// 改写连接属性、短路或返回错误；默认继续有位置链，末端才调用驱动。
    async fn connection_connect(
        &self,
        chain: &mut PhysicalConnectionConnectFilterChain<'_>,
        properties: &mut std::collections::HashMap<String, String>,
    ) -> Result<PhysicalConnectionConnectResult, DruidError> {
        chain.connection_connect(properties).await
    }

    /// 包围一次真实物理连接关闭。
    ///
    /// 对应 Java：`Filter#connection_close(FilterChain, ConnectionProxy)`。该
    /// hook 只在驱动连接实际销毁时执行，不得与池化连接的逻辑 `close`/归还
    /// 混为一谈。默认继续有位置调用链，末端才调用驱动工厂关闭原始连接。
    async fn connection_close(
        &self,
        chain: &mut PhysicalConnectionCloseFilterChain<'_>,
    ) -> Result<(), DruidError> {
        chain.connection_close().await
    }

    /// 通用前置拦截（对应 Filter 的 before-execute 语义）。
    async fn before(&self, ctx: &mut ExecContext<'_>) -> Result<(), DruidError>;

    /// 在创建物理 PreparedStatement/CallableStatement 前检查并改写 SQL。
    ///
    /// 对应 Java `Filter#connection_prepareStatement/prepareCall` around-chain。
    /// 默认保持文本不变；返回值必须作为物理 prepare 和缓存键的共同 SQL。
    fn prepare_statement_sql(&self, sql: &str) -> Result<String, DruidError> {
        Ok(sql.to_owned())
    }

    /// 在普通 Statement 保存 batch SQL 前检查并改写文本。
    ///
    /// 对应 Java `Filter#statement_addBatch` around-chain；改写必须发生在
    /// `PhysicalStatement#add_batch` 前，而不是 executeBatch 时临时替换副本。
    fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        Ok(sql.to_owned())
    }

    /// 本 Filter 的 before 已成功，但后续 Filter 在执行前短路时逆序回调。
    ///
    /// 对应 Java around-chain 在下游异常时的栈展开。默认没有待清理状态。
    async fn before_execute_error(
        &self,
        _context: &ExecContext<'_>,
        _error: &DruidError,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 应用数据源连接属性。
    ///
    /// 对应 Java：`Filter#configFromProperties(Properties)`。Java 在
    /// `setConnectProperties` 时对当时已存在的显式 Filter 依注册顺序调用；
    /// Rust 使用共享引用配合 Filter 内部同步原语保留同一时序。
    fn config_from_properties(
        &self,
        _properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 应用宿主 system properties。
    ///
    /// Java Filter 对 system properties 的时序并不统一；默认不应用，由
    /// `StatFilter` 等明确读取系统属性的对象覆盖。
    fn config_from_system_properties(
        &self,
        _properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 批处理前置拦截。
    ///
    /// 默认适配到一次通用 `before`，不得逐条触发 Filter；需要观察原始批次的
    /// Filter 可覆盖本方法。
    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        let params = context
            .parameter_sets
            .last()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut execute_context = ExecContext {
            connection_id: context.connection_id,
            statement_id: context.statement_id,
            sql: context.sql.to_owned(),
            params,
            prepared_parameters: context
                .prepared_parameter_sets
                .and_then(|sets| sets.last())
                .map(Vec::as_slice),
            data_source: context.data_source,
            start: context.start,
            fingerprint: context.fingerprint,
            in_transaction: context.in_transaction,
            operation: ExecOperation::Batch,
        };
        self.before(&mut execute_context).await
    }

    /// batch 前置链在后续 Filter 短路时逆序回调。
    async fn before_batch_error(
        &self,
        context: &BatchExecContext<'_>,
        error: &DruidError,
    ) -> Result<(), DruidError> {
        let params = context
            .parameter_sets
            .last()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let execute_context = ExecContext {
            connection_id: context.connection_id,
            statement_id: context.statement_id,
            sql: context.sql.to_owned(),
            params,
            prepared_parameters: context
                .prepared_parameter_sets
                .and_then(|sets| sets.last())
                .map(Vec::as_slice),
            data_source: context.data_source,
            start: context.start,
            fingerprint: context.fingerprint,
            in_transaction: context.in_transaction,
            operation: ExecOperation::Batch,
        };
        self.before_execute_error(&execute_context, error).await
    }

    /// 连接事件拦截（对应 Filter 的 connection_* 系列 hook）。
    /// 返回 Ok(()) 放行，Err 短路。
    async fn on_connection_event(&self, _event: &ConnectionEvent) -> Result<(), DruidError> {
        Ok(()) // 默认放行
    }

    /// 带连接身份的事件入口；默认保持旧 Filter 实现兼容。
    async fn on_connection_event_context(
        &self,
        context: &ConnectionEventContext<'_>,
    ) -> Result<(), DruidError> {
        self.on_connection_event(context.event).await
    }

    /// 包围 `Connection#getWarnings()`。
    ///
    /// 对应 Java：`Filter#connection_getWarnings`。默认实现继续调用链；Filter
    /// 可以短路或改写警告链，不能退化为只读事件。
    async fn connection_get_warnings(
        &self,
        chain: &mut ConnectionWarningFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        chain.connection_get_warnings().await
    }

    /// 包围 `Connection#clearWarnings()`。
    ///
    /// 对应 Java：`Filter#connection_clearWarnings`。
    async fn connection_clear_warnings(
        &self,
        chain: &mut ConnectionWarningFilterChain<'_>,
    ) -> Result<(), DruidError> {
        chain.connection_clear_warnings().await
    }

    /// 包围 `Connection#getMetaData()`。
    ///
    /// 对应 Java：`Filter#connection_getMetaData`。链按注册顺序执行，末端才向
    /// 当前物理连接借用 metadata；Filter 可阻断或用保持同一连接生命周期的
    /// Adapter 包装返回值。
    fn connection_get_meta_data<'filters, 'connection>(
        &self,
        chain: ConnectionDatabaseMetaDataFilterChain<'filters, 'connection>,
    ) -> Result<Box<dyn PhysicalDatabaseMetaData + 'connection>, DruidError> {
        chain.connection_get_meta_data()
    }

    /// 语句事件拦截（对应 Filter 的 statement_* / preparedStatement_* 系列 hook）。
    async fn on_statement_event(&self, _event: &StatementEvent) -> Result<(), DruidError> {
        Ok(()) // 默认放行
    }

    /// 带 Connection/Statement 身份的事件入口；默认转发旧事件 hook。
    async fn on_statement_event_context(
        &self,
        context: &StatementEventContext<'_>,
    ) -> Result<(), DruidError> {
        self.on_statement_event(context.event).await
    }

    /// 同步 Statement close 后置事件。
    ///
    /// Rust 的 Statement/PreparedStatement `close` 与 Drop 状态机是同步的；
    /// 该 hook 只允许观察已经完成的逻辑关闭，不得执行异步 I/O。
    fn on_statement_close_context(
        &self,
        _context: &StatementEventContext<'_>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 包围 `Statement#getWarnings()`；PreparedStatement 继承同一调用链。
    ///
    /// 对应 Java：`Filter#statement_getWarnings`。
    async fn statement_get_warnings(
        &self,
        chain: &mut StatementWarningFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        chain.statement_get_warnings().await
    }

    /// 包围 `Statement#clearWarnings()`；PreparedStatement 继承同一调用链。
    ///
    /// 对应 Java：`Filter#statement_clearWarnings`。
    async fn statement_clear_warnings(
        &self,
        chain: &mut StatementWarningFilterChain<'_>,
    ) -> Result<(), DruidError> {
        chain.statement_clear_warnings().await
    }

    /// 结果集事件拦截（对应 Filter 的 resultSet_* 系列 hook）。
    async fn on_result_set_event(&self, _event: &ResultSetEvent) -> Result<(), DruidError> {
        Ok(()) // 默认放行
    }

    /// 过滤器生命周期（对应 Filter.init()）。
    async fn init(&self) -> Result<(), DruidError> {
        Ok(())
    }

    /// 过滤器销毁（对应 Filter.destroy()）。
    async fn destroy(&self) -> Result<(), DruidError> {
        Ok(())
    }
}

/// 后置 Filter trait。
///
/// 对应 DruidJava `Filter` 接口的 after-execute hook。
/// 即使 SQL 执行失败也会调用。
#[async_trait::async_trait]
pub trait AfterFilter: Send + Sync {
    /// 返回 Filter 名称。
    fn name(&self) -> &str;

    /// 后置拦截。
    async fn after(
        &self,
        ctx: &ExecContext<'_>,
        result: &Result<super::connection::ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError>;

    /// 批处理后置拦截。
    ///
    /// 默认把成功计数聚合成一次 `ExecResult` 后调用通用 `after`。Java Filter
    /// 链对整个 batch 只进入和退出一次；需要逐项计数的 Filter 应覆盖本方法。
    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let execute_result = match result {
            Ok(update_counts) => Ok(super::connection::ExecResult {
                rows_affected: update_counts
                    .iter()
                    .filter_map(|count| u64::try_from(*count).ok())
                    .sum(),
                last_insert_id: None,
                row_count: None,
            }),
            Err(error) => Err(error.clone()),
        };
        let params = context
            .parameter_sets
            .last()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let execute_context = ExecContext {
            connection_id: context.connection_id,
            statement_id: context.statement_id,
            sql: context.sql.to_owned(),
            params,
            prepared_parameters: context
                .prepared_parameter_sets
                .and_then(|sets| sets.last())
                .map(Vec::as_slice),
            data_source: context.data_source,
            start: context.start,
            fingerprint: None,
            in_transaction: context.in_transaction,
            operation: ExecOperation::Batch,
        };
        self.after(&execute_context, &execute_result, elapsed).await
    }

    /// 物理连接事件完成后的回调。
    ///
    /// 对应 Java Filter 在 `chain.connection_*` 成功返回后执行的逻辑。
    async fn after_connection_event(
        &self,
        _event: &ConnectionEvent,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 带连接身份的后置事件；默认转发旧 hook。
    async fn after_connection_event_context(
        &self,
        context: &ConnectionEventContext<'_>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.after_connection_event(context.event, elapsed).await
    }
}

// ── 扩展事件枚举（V2+ 阶段）────────────────────────────────────

/// Statement 属性查询/设置事件，对应 DruidJava 的 statement_set* / statement_get* hook。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementPropertyEvent {
    /// 设置查询超时（对应 statement_setQueryTimeout）
    SetQueryTimeout(i32),
    /// 获取查询超时（对应 statement_getQueryTimeout）
    GetQueryTimeout,
    /// 获取更新计数（对应 statement_getUpdateCount）
    GetUpdateCount,
    /// 设置最大行数（对应 statement_setMaxRows）
    SetMaxRows(i32),
    /// 获取最大行数（对应 statement_getMaxRows）
    GetMaxRows,
    /// 设置最大字段大小（对应 statement_setMaxFieldSize）
    SetMaxFieldSize(i32),
    /// 获取最大字段大小（对应 statement_getMaxFieldSize）
    GetMaxFieldSize,
    /// 设置获取方向（对应 statement_setFetchDirection）
    SetFetchDirection(i32),
    /// 获取获取方向（对应 statement_getFetchDirection）
    GetFetchDirection,
    /// 设置获取大小（对应 statement_setFetchSize）
    SetFetchSize(i32),
    /// 获取获取大小（对应 statement_getFetchSize）
    GetFetchSize,
    /// 检查是否池化（对应 statement_isPoolable）
    IsPoolable,
    /// 检查是否关闭（对应 statement_isClosed）
    IsClosed,
    /// 获取更多结果（对应 statement_getMoreResults）
    GetMoreResults,
    /// 获取结果集并发性（对应 statement_getResultSetConcurrency）
    GetResultSetConcurrency,
    /// 获取结果集类型（对应 statement_getResultSetType）
    GetResultSetType,
    /// 获取结果集保持性（对应 statement_getResultSetHoldability）
    GetResultSetHoldability,
    /// 获取生成的键（对应 statement_getGeneratedKeys）
    GetGeneratedKeys,
    /// 清除警告（对应 statement_clearWarnings）
    ClearWarnings,
    /// 重命名结果集（对应 statement_setCursorName）
    SetCursorName(String),
    /// 添加批次（对应 statement_addBatch）
    AddBatch(String),
}

/// Clob 事件，对应 DruidJava 的 clob_* hook。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClobEvent {
    /// 获取长度（对应 clob_length）
    Length,
    /// 获取子串（对应 clob_getSubString）
    GetSubString(i64, i32),
    /// 设置字符串（对应 clob_setString）
    SetString(i64, String),
    /// 截断（对应 clob_truncate）
    Truncate(i64),
    /// 释放（对应 clob_free）
    Free,
}

/// DataSource 级别事件，对应 DruidJava 的 dataSource_* hook。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataSourceEvent {
    /// 获取连接（对应 dataSource_getConnection）
    GetConnection,
    /// 获取连接带认证（对应 dataSource_getConnection(user, pass)）
    GetConnectionWithAuth(String, String),
    /// 释放连接（对应 dataSource_releaseConnection）
    ReleaseConnection,
    /// 日志记录（对应 dataSource_log）
    Log(String),
}

// ── 扩展 BeforeFilter（V2+ 阶段）────────────────────────────

/// 扩展 Filter hook（V2+ 阶段），覆盖 DruidJava 的全部 384 个 hook 中的
/// statement 属性、clob 和 dataSource 级别事件。
#[async_trait::async_trait]
pub trait ExtendedFilter: Send + Sync {
    /// Statement 属性事件。
    async fn on_statement_property_event(
        &self,
        _event: &StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// Clob 事件。
    async fn on_clob_event(&self, _event: &ClobEvent) -> Result<(), DruidError> {
        Ok(())
    }

    /// DataSource 级别事件。
    async fn on_datasource_event(&self, _event: &DataSourceEvent) -> Result<(), DruidError> {
        Ok(())
    }

    /// 过滤器配置（对应 Filter.configFromProperties）。
    async fn config_from_properties(
        &mut self,
        _properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 过滤器是否匹配给定接口（对应 Filter.isWrapperFor）。
    fn is_wrapper_for(&self, _type_name: &str) -> bool {
        false
    }
}
