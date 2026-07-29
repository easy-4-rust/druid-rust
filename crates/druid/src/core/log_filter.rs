//! 对应 Java：`com.alibaba.druid.filter.logging.LogFilter`。
//! 来源文件：`core/src/main/java/com/alibaba/druid/filter/logging/LogFilter.java`。

use super::{
    AfterFilter, BatchExecContext, BeforeFilter, ConnectionEvent, DruidError, ExecContext,
    ExecOperation, ExecResult, ResultSetFilter, ResultSetFilterChain, ResultSetFilterContext,
    StatementEvent,
};
use crate::sql::SqlFormatOption;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Java 日志 Filter 家族共享的 tracing 语义实现。
///
/// Java 的 Log4j、Log4j2、SLF4J 和 Commons Logging 子类只负责选择日志后端；
/// Rust 将后端统一为 `tracing`，但保留四个 logger 名称、各生命周期开关、参数/
/// 可执行 SQL 开关、错误开关以及 ResultSet 事件开关。当前 Filter 协议尚未携带
/// Java proxy id 和所有列值，无法表达的格式细节继续在迁移台账中保持 PARTIAL。
pub struct LogFilter {
    backend_name: &'static str,
    data_source_logger_name: RwLock<String>,
    connection_logger_name: RwLock<String>,
    statement_logger_name: RwLock<String>,
    result_set_logger_name: RwLock<String>,
    connection_connect_before_log_enabled: AtomicBool,
    connection_connect_after_log_enabled: AtomicBool,
    connection_commit_after_log_enabled: AtomicBool,
    connection_rollback_after_log_enabled: AtomicBool,
    connection_close_after_log_enabled: AtomicBool,
    statement_create_after_log_enabled: AtomicBool,
    statement_prepare_after_log_enabled: AtomicBool,
    statement_prepare_call_after_log_enabled: AtomicBool,
    statement_execute_after_log_enabled: AtomicBool,
    statement_execute_query_after_log_enabled: AtomicBool,
    statement_execute_update_after_log_enabled: AtomicBool,
    statement_execute_batch_after_log_enabled: AtomicBool,
    statement_close_after_log_enabled: AtomicBool,
    statement_parameter_clear_log_enabled: AtomicBool,
    statement_parameter_set_log_enabled: AtomicBool,
    statement_executable_sql_log_enabled: AtomicBool,
    result_set_next_after_log_enabled: AtomicBool,
    result_set_open_after_log_enabled: AtomicBool,
    result_set_close_after_log_enabled: AtomicBool,
    data_source_log_enabled: AtomicBool,
    connection_log_enabled: AtomicBool,
    connection_log_error_enabled: AtomicBool,
    statement_log_enabled: AtomicBool,
    statement_log_error_enabled: AtomicBool,
    result_set_log_enabled: AtomicBool,
    result_set_log_error_enabled: AtomicBool,
    statement_sql_format_option: RwLock<SqlFormatOption>,
    statement_sql_pretty_format: AtomicBool,
}

impl LogFilter {
    /// 创建 Java 默认配置的 tracing LogFilter。
    #[must_use]
    pub fn new() -> Self {
        Self {
            backend_name: "tracing",
            data_source_logger_name: RwLock::new("druid.sql.DataSource".to_owned()),
            connection_logger_name: RwLock::new("druid.sql.Connection".to_owned()),
            statement_logger_name: RwLock::new("druid.sql.Statement".to_owned()),
            result_set_logger_name: RwLock::new("druid.sql.ResultSet".to_owned()),
            connection_connect_before_log_enabled: AtomicBool::new(true),
            connection_connect_after_log_enabled: AtomicBool::new(true),
            connection_commit_after_log_enabled: AtomicBool::new(true),
            connection_rollback_after_log_enabled: AtomicBool::new(true),
            connection_close_after_log_enabled: AtomicBool::new(true),
            statement_create_after_log_enabled: AtomicBool::new(true),
            statement_prepare_after_log_enabled: AtomicBool::new(true),
            statement_prepare_call_after_log_enabled: AtomicBool::new(true),
            statement_execute_after_log_enabled: AtomicBool::new(true),
            statement_execute_query_after_log_enabled: AtomicBool::new(true),
            statement_execute_update_after_log_enabled: AtomicBool::new(true),
            statement_execute_batch_after_log_enabled: AtomicBool::new(true),
            statement_close_after_log_enabled: AtomicBool::new(true),
            statement_parameter_clear_log_enabled: AtomicBool::new(true),
            statement_parameter_set_log_enabled: AtomicBool::new(true),
            statement_executable_sql_log_enabled: AtomicBool::new(false),
            result_set_next_after_log_enabled: AtomicBool::new(true),
            result_set_open_after_log_enabled: AtomicBool::new(true),
            result_set_close_after_log_enabled: AtomicBool::new(true),
            data_source_log_enabled: AtomicBool::new(true),
            connection_log_enabled: AtomicBool::new(true),
            connection_log_error_enabled: AtomicBool::new(true),
            statement_log_enabled: AtomicBool::new(true),
            statement_log_error_enabled: AtomicBool::new(true),
            result_set_log_enabled: AtomicBool::new(true),
            result_set_log_error_enabled: AtomicBool::new(true),
            statement_sql_format_option: RwLock::new(SqlFormatOption::new(false, true, false)),
            statement_sql_pretty_format: AtomicBool::new(false),
        }
    }

    /// 按 Java `configFromProperties` 的精确键和大小写规则更新七个全局开关。
    pub fn config_from_properties(&self, properties: &HashMap<String, String>) {
        Self::set_from_java_property(properties, "druid.log.conn", &self.connection_log_enabled);
        Self::set_from_java_property(properties, "druid.log.stmt", &self.statement_log_enabled);
        Self::set_from_java_property(properties, "druid.log.rs", &self.result_set_log_enabled);
        Self::set_from_java_property(
            properties,
            "druid.log.stmt.executableSql",
            &self.statement_executable_sql_log_enabled,
        );
        Self::set_from_java_property(
            properties,
            "druid.log.conn.logError",
            &self.connection_log_error_enabled,
        );
        Self::set_from_java_property(
            properties,
            "druid.log.stmt.logError",
            &self.statement_log_error_enabled,
        );
        Self::set_from_java_property(
            properties,
            "druid.log.rs.logError",
            &self.result_set_log_error_enabled,
        );
    }

    fn set_from_java_property(
        properties: &HashMap<String, String>,
        key: &str,
        target: &AtomicBool,
    ) {
        match properties.get(key).map(String::as_str) {
            Some("true") => target.store(true, Ordering::Release),
            Some("false") => target.store(false, Ordering::Release),
            _ => {}
        }
    }

    fn enabled(flag: &AtomicBool) -> bool {
        flag.load(Ordering::Acquire)
    }

    /// 返回日志后端语义名称。
    #[must_use]
    pub const fn backend_name(&self) -> &'static str {
        self.backend_name
    }

    /// 返回 DataSource logger 名称。
    #[must_use]
    pub fn data_source_logger_name(&self) -> String {
        self.data_source_logger_name.read().clone()
    }

    /// 设置 DataSource logger 名称。
    pub fn set_data_source_logger_name(&self, logger_name: impl Into<String>) {
        *self.data_source_logger_name.write() = logger_name.into();
    }

    /// 返回 Connection logger 名称。
    #[must_use]
    pub fn connection_logger_name(&self) -> String {
        self.connection_logger_name.read().clone()
    }

    /// 设置 Connection logger 名称。
    pub fn set_connection_logger_name(&self, logger_name: impl Into<String>) {
        *self.connection_logger_name.write() = logger_name.into();
    }

    /// 返回 Statement logger 名称。
    #[must_use]
    pub fn statement_logger_name(&self) -> String {
        self.statement_logger_name.read().clone()
    }

    /// 设置 Statement logger 名称。
    pub fn set_statement_logger_name(&self, logger_name: impl Into<String>) {
        *self.statement_logger_name.write() = logger_name.into();
    }

    /// 返回 ResultSet logger 名称。
    #[must_use]
    pub fn result_set_logger_name(&self) -> String {
        self.result_set_logger_name.read().clone()
    }

    /// 设置 ResultSet logger 名称。
    pub fn set_result_set_logger_name(&self, logger_name: impl Into<String>) {
        *self.result_set_logger_name.write() = logger_name.into();
    }

    /// 是否记录 DataSource。
    #[must_use]
    pub fn is_data_source_log_enabled(&self) -> bool {
        Self::enabled(&self.data_source_log_enabled)
    }

    /// 设置是否记录 DataSource。
    pub fn set_data_source_log_enabled(&self, enabled: bool) {
        self.data_source_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 Connection。
    #[must_use]
    pub fn is_connection_log_enabled(&self) -> bool {
        Self::enabled(&self.connection_log_enabled)
    }

    /// 设置是否记录 Connection。
    pub fn set_connection_log_enabled(&self, enabled: bool) {
        self.connection_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 Connection 错误。
    #[must_use]
    pub fn is_connection_log_error_enabled(&self) -> bool {
        Self::enabled(&self.connection_log_error_enabled)
    }

    /// 设置是否记录 Connection 错误。
    pub fn set_connection_log_error_enabled(&self, enabled: bool) {
        self.connection_log_error_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 connect 前置事件。
    #[must_use]
    pub fn is_connection_connect_before_log_enabled(&self) -> bool {
        self.is_connection_log_enabled()
            && Self::enabled(&self.connection_connect_before_log_enabled)
    }

    /// 设置 connect 前置事件开关。
    pub fn set_connection_connect_before_log_enabled(&self, enabled: bool) {
        self.connection_connect_before_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 connect 后置事件。
    #[must_use]
    pub fn is_connection_connect_after_log_enabled(&self) -> bool {
        self.is_connection_log_enabled()
            && Self::enabled(&self.connection_connect_after_log_enabled)
    }

    /// 设置 connect 后置事件开关。
    pub fn set_connection_connect_after_log_enabled(&self, enabled: bool) {
        self.connection_connect_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 close 后置事件。
    #[must_use]
    pub fn is_connection_close_after_log_enabled(&self) -> bool {
        self.is_connection_log_enabled() && Self::enabled(&self.connection_close_after_log_enabled)
    }

    /// 设置 close 后置事件开关。
    pub fn set_connection_close_after_log_enabled(&self, enabled: bool) {
        self.connection_close_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 commit 后置事件。
    #[must_use]
    pub fn is_connection_commit_after_log_enabled(&self) -> bool {
        self.is_connection_log_enabled() && Self::enabled(&self.connection_commit_after_log_enabled)
    }

    /// 设置 commit 后置事件开关。
    pub fn set_connection_commit_after_log_enabled(&self, enabled: bool) {
        self.connection_commit_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 rollback 后置事件。
    #[must_use]
    pub fn is_connection_rollback_after_log_enabled(&self) -> bool {
        self.is_connection_log_enabled()
            && Self::enabled(&self.connection_rollback_after_log_enabled)
    }

    /// 设置 rollback 后置事件开关。
    pub fn set_connection_rollback_after_log_enabled(&self, enabled: bool) {
        self.connection_rollback_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 Statement。
    #[must_use]
    pub fn is_statement_log_enabled(&self) -> bool {
        Self::enabled(&self.statement_log_enabled)
    }

    /// 设置是否记录 Statement。
    pub fn set_statement_log_enabled(&self, enabled: bool) {
        self.statement_log_enabled.store(enabled, Ordering::Release);
    }

    /// 是否记录 Statement 错误。
    #[must_use]
    pub fn is_statement_log_error_enabled(&self) -> bool {
        Self::enabled(&self.statement_log_error_enabled)
    }

    /// 设置是否记录 Statement 错误。
    pub fn set_statement_log_error_enabled(&self, enabled: bool) {
        self.statement_log_error_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 Statement create 后置事件。
    #[must_use]
    pub fn is_statement_create_after_log_enabled(&self) -> bool {
        self.is_statement_log_enabled() && Self::enabled(&self.statement_create_after_log_enabled)
    }

    /// 设置 Statement create 后置事件。
    pub fn set_statement_create_after_log_enabled(&self, enabled: bool) {
        self.statement_create_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 prepare 后置事件。
    #[must_use]
    pub fn is_statement_prepare_after_log_enabled(&self) -> bool {
        self.is_statement_log_enabled() && Self::enabled(&self.statement_prepare_after_log_enabled)
    }

    /// 设置 prepare 后置事件。
    pub fn set_statement_prepare_after_log_enabled(&self, enabled: bool) {
        self.statement_prepare_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 prepareCall 后置事件。
    #[must_use]
    pub fn is_statement_prepare_call_after_log_enabled(&self) -> bool {
        self.is_statement_log_enabled()
            && Self::enabled(&self.statement_prepare_call_after_log_enabled)
    }

    /// 设置 prepareCall 后置事件。
    pub fn set_statement_prepare_call_after_log_enabled(&self, enabled: bool) {
        self.statement_prepare_call_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 close 后置事件。
    #[must_use]
    pub fn is_statement_close_after_log_enabled(&self) -> bool {
        self.is_statement_log_enabled() && Self::enabled(&self.statement_close_after_log_enabled)
    }

    /// 设置 close 后置事件。
    pub fn set_statement_close_after_log_enabled(&self, enabled: bool) {
        self.statement_close_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 generic execute 后置事件。
    #[must_use]
    pub fn is_statement_execute_after_log_enabled(&self) -> bool {
        self.is_statement_log_enabled() && Self::enabled(&self.statement_execute_after_log_enabled)
    }

    /// 设置 generic execute 后置事件。
    pub fn set_statement_execute_after_log_enabled(&self, enabled: bool) {
        self.statement_execute_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 executeQuery 后置事件。
    #[must_use]
    pub fn is_statement_execute_query_after_log_enabled(&self) -> bool {
        self.is_statement_log_enabled()
            && Self::enabled(&self.statement_execute_query_after_log_enabled)
    }

    /// 设置 executeQuery 后置事件。
    pub fn set_statement_execute_query_after_log_enabled(&self, enabled: bool) {
        self.statement_execute_query_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 executeUpdate 后置事件。
    #[must_use]
    pub fn is_statement_execute_update_after_log_enabled(&self) -> bool {
        self.is_statement_log_enabled()
            && Self::enabled(&self.statement_execute_update_after_log_enabled)
    }

    /// 设置 executeUpdate 后置事件。
    pub fn set_statement_execute_update_after_log_enabled(&self, enabled: bool) {
        self.statement_execute_update_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 executeBatch 后置事件。
    #[must_use]
    pub fn is_statement_execute_batch_after_log_enabled(&self) -> bool {
        self.is_statement_log_enabled()
            && Self::enabled(&self.statement_execute_batch_after_log_enabled)
    }

    /// 设置 executeBatch 后置事件。
    pub fn set_statement_execute_batch_after_log_enabled(&self, enabled: bool) {
        self.statement_execute_batch_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录参数设置。
    #[must_use]
    pub fn is_statement_parameter_set_log_enabled(&self) -> bool {
        self.is_statement_log_enabled() && Self::enabled(&self.statement_parameter_set_log_enabled)
    }

    /// 设置参数日志开关。
    pub fn set_statement_parameter_set_log_enabled(&self, enabled: bool) {
        self.statement_parameter_set_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 兼容旧 Rust API：参数日志与 Java parameter-set 开关相同。
    pub fn set_statement_parameter_log_enabled(&self, enabled: bool) {
        self.set_statement_parameter_set_log_enabled(enabled);
    }

    /// 是否记录参数清理。
    #[must_use]
    pub fn is_statement_parameter_clear_log_enabled(&self) -> bool {
        self.is_statement_log_enabled()
            && Self::enabled(&self.statement_parameter_clear_log_enabled)
    }

    /// 设置参数清理日志开关。
    pub fn set_statement_parameter_clear_log_enabled(&self, enabled: bool) {
        self.statement_parameter_clear_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录可执行 SQL。
    #[must_use]
    pub fn is_statement_executable_sql_log_enabled(&self) -> bool {
        Self::enabled(&self.statement_executable_sql_log_enabled)
    }

    /// 设置可执行 SQL 日志开关。
    pub fn set_statement_executable_sql_log_enabled(&self, enabled: bool) {
        self.statement_executable_sql_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 ResultSet。
    #[must_use]
    pub fn is_result_set_log_enabled(&self) -> bool {
        Self::enabled(&self.result_set_log_enabled)
    }

    /// 设置是否记录 ResultSet。
    pub fn set_result_set_log_enabled(&self, enabled: bool) {
        self.result_set_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 ResultSet 错误。
    #[must_use]
    pub fn is_result_set_log_error_enabled(&self) -> bool {
        Self::enabled(&self.result_set_log_error_enabled)
    }

    /// 设置 ResultSet 错误日志开关。
    pub fn set_result_set_log_error_enabled(&self, enabled: bool) {
        self.result_set_log_error_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 ResultSet open。
    #[must_use]
    pub fn is_result_set_open_after_log_enabled(&self) -> bool {
        self.is_result_set_log_enabled() && Self::enabled(&self.result_set_open_after_log_enabled)
    }

    /// 设置 ResultSet open 日志开关。
    pub fn set_result_set_open_after_log_enabled(&self, enabled: bool) {
        self.result_set_open_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 ResultSet next。
    #[must_use]
    pub fn is_result_set_next_after_log_enabled(&self) -> bool {
        self.is_result_set_log_enabled() && Self::enabled(&self.result_set_next_after_log_enabled)
    }

    /// 设置 ResultSet next 日志开关。
    pub fn set_result_set_next_after_log_enabled(&self, enabled: bool) {
        self.result_set_next_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 ResultSet close。
    #[must_use]
    pub fn is_result_set_close_after_log_enabled(&self) -> bool {
        self.is_result_set_log_enabled() && Self::enabled(&self.result_set_close_after_log_enabled)
    }

    /// 设置 ResultSet close 日志开关。
    pub fn set_result_set_close_after_log_enabled(&self, enabled: bool) {
        self.result_set_close_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 返回 SQL 格式选项。
    #[must_use]
    pub fn statement_sql_format_option(&self) -> SqlFormatOption {
        *self.statement_sql_format_option.read()
    }

    /// 设置 SQL 格式选项。
    pub fn set_statement_sql_format_option(&self, option: SqlFormatOption) {
        *self.statement_sql_format_option.write() = option;
    }

    /// 是否启用 legacy pretty SQL 日志。
    #[must_use]
    pub fn is_statement_sql_pretty_format(&self) -> bool {
        Self::enabled(&self.statement_sql_pretty_format)
    }

    /// 设置 legacy pretty SQL 日志。
    pub fn set_statement_sql_pretty_format(&self, enabled: bool) {
        self.statement_sql_pretty_format
            .store(enabled, Ordering::Release);
    }

    fn operation_success_enabled(&self, operation: ExecOperation) -> bool {
        match operation {
            ExecOperation::Execute => self.is_statement_execute_after_log_enabled(),
            ExecOperation::Query => self.is_statement_execute_query_after_log_enabled(),
            ExecOperation::Update => self.is_statement_execute_update_after_log_enabled(),
            ExecOperation::Batch => self.is_statement_execute_batch_after_log_enabled(),
        }
    }
}

impl Default for LogFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl BeforeFilter for LogFilter {
    fn name(&self) -> &str {
        "log"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        if self.is_statement_parameter_set_log_enabled() && !context.params.is_empty() {
            let logger = self.statement_logger_name();
            tracing::debug!(
                backend = self.backend_name,
                logger,
                data_source = context.data_source,
                parameters = ?context.params,
                "statement parameters"
            );
        }
        Ok(())
    }

    async fn on_connection_event(&self, event: &ConnectionEvent) -> Result<(), DruidError> {
        if matches!(event, ConnectionEvent::Connect)
            && self.is_connection_connect_before_log_enabled()
        {
            let logger = self.connection_logger_name();
            tracing::debug!(
                backend = self.backend_name,
                logger,
                "connection connect before"
            );
        }
        Ok(())
    }

    async fn on_statement_event(&self, event: &StatementEvent) -> Result<(), DruidError> {
        let enabled = match event {
            StatementEvent::CreateStatement => self.is_statement_create_after_log_enabled(),
            StatementEvent::PrepareStatement(_) => self.is_statement_prepare_after_log_enabled(),
            StatementEvent::PrepareCall(_) => self.is_statement_prepare_call_after_log_enabled(),
            StatementEvent::Close => self.is_statement_close_after_log_enabled(),
            _ => false,
        };
        if enabled {
            let logger = self.statement_logger_name();
            tracing::debug!(
                backend = self.backend_name,
                logger,
                ?event,
                "statement event"
            );
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for LogFilter {
    fn name(&self) -> &str {
        "log"
    }

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let logger = self.statement_logger_name();
        match result {
            Ok(result) if self.operation_success_enabled(context.operation) => {
                tracing::debug!(
                    backend = self.backend_name,
                    logger,
                    data_source = context.data_source,
                    sql = context.sql,
                    parameters = ?context.params,
                    executable_sql = self.is_statement_executable_sql_log_enabled(),
                    elapsed_ms = elapsed.as_millis(),
                    rows_affected = result.rows_affected,
                    row_count = result.row_count,
                    operation = ?context.operation,
                    "statement execute after"
                );
            }
            Err(error) if self.is_statement_log_error_enabled() => {
                tracing::error!(
                    backend = self.backend_name,
                    logger,
                    data_source = context.data_source,
                    sql = context.sql,
                    parameters = ?context.params,
                    elapsed_ms = elapsed.as_millis(),
                    operation = ?context.operation,
                    %error,
                    "statement execute error"
                );
            }
            _ => {}
        }
        Ok(())
    }

    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let logger = self.statement_logger_name();
        match result {
            Ok(update_counts) if self.is_statement_execute_batch_after_log_enabled() => {
                tracing::debug!(
                    backend = self.backend_name,
                    logger,
                    data_source = context.data_source,
                    sql = context.sql,
                    update_counts = ?update_counts,
                    elapsed_ms = elapsed.as_millis(),
                    "statement batch execute after"
                );
            }
            Err(error) if self.is_statement_log_error_enabled() => {
                tracing::error!(
                    backend = self.backend_name,
                    logger,
                    data_source = context.data_source,
                    sql = context.sql,
                    elapsed_ms = elapsed.as_millis(),
                    %error,
                    "statement batch execute error"
                );
            }
            _ => {}
        }
        Ok(())
    }

    async fn after_connection_close(&self) -> Result<(), DruidError> {
        if self.is_connection_close_after_log_enabled() {
            let logger = self.connection_logger_name();
            tracing::debug!(backend = self.backend_name, logger, "connection closed");
        }
        Ok(())
    }

    async fn after_connection_event(
        &self,
        event: &ConnectionEvent,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let enabled = match event {
            ConnectionEvent::Connect => self.is_connection_connect_after_log_enabled(),
            ConnectionEvent::Commit => self.is_connection_commit_after_log_enabled(),
            ConnectionEvent::Rollback => self.is_connection_rollback_after_log_enabled(),
            ConnectionEvent::Close => self.is_connection_close_after_log_enabled(),
            _ => self.is_connection_log_enabled(),
        };
        if enabled {
            let logger = self.connection_logger_name();
            tracing::debug!(
                backend = self.backend_name,
                logger,
                ?event,
                elapsed_ms = elapsed.as_millis(),
                "connection event after"
            );
        }
        Ok(())
    }
}

impl ResultSetFilter for LogFilter {
    fn result_set_open_after(&self, context: &ResultSetFilterContext) -> Result<(), DruidError> {
        if self.is_result_set_open_after_log_enabled() {
            let logger = self.result_set_logger_name();
            tracing::debug!(
                backend = self.backend_name,
                logger,
                fetch_row_count = context.fetch_row_count(),
                "result set open"
            );
        }
        Ok(())
    }

    fn result_set_next(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<bool, DruidError> {
        let more_rows = chain.result_set_next();
        match more_rows {
            Ok(true) if self.is_result_set_next_after_log_enabled() => {
                let logger = self.result_set_logger_name();
                tracing::debug!(backend = self.backend_name, logger, "result set next");
            }
            Err(ref error) if self.is_result_set_log_error_enabled() => {
                let logger = self.result_set_logger_name();
                tracing::error!(backend = self.backend_name, logger, %error, "result set next error");
            }
            _ => {}
        }
        more_rows
    }

    fn result_set_close(&self, chain: &mut ResultSetFilterChain<'_>) -> Result<(), DruidError> {
        let result = chain.result_set_close();
        match result {
            Ok(()) if self.is_result_set_close_after_log_enabled() => {
                let logger = self.result_set_logger_name();
                tracing::debug!(backend = self.backend_name, logger, "result set closed");
            }
            Err(ref error) if self.is_result_set_log_error_enabled() => {
                let logger = self.result_set_logger_name();
                tracing::error!(backend = self.backend_name, logger, %error, "result set close error");
            }
            _ => {}
        }
        result
    }
}
