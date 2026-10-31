//! `LogFilter` 的类别与开关访问器。
//!
//! 拆分自 `log_filter.rs`：本文件只承载 Java 同名 getter/setter，
//! 日志事件实现仍留在父模块。

use super::*;

impl LogFilter {
    /// 返回 `DataSource` 结构化事件分类。
    #[must_use]
    pub fn data_source_category(&self) -> String {
        self.data_source_category.read().clone()
    }

    /// 设置 `DataSource` 结构化事件分类。
    pub fn set_data_source_category(&self, category: impl Into<String>) {
        *self.data_source_category.write() = category.into();
    }

    /// 返回 Connection 结构化事件分类。
    #[must_use]
    pub fn connection_category(&self) -> String {
        self.connection_category.read().clone()
    }

    /// 设置 Connection 结构化事件分类。
    pub fn set_connection_category(&self, category: impl Into<String>) {
        *self.connection_category.write() = category.into();
    }

    /// 返回 Statement 结构化事件分类。
    #[must_use]
    pub fn statement_category(&self) -> String {
        self.statement_category.read().clone()
    }

    /// 设置 Statement 结构化事件分类。
    pub fn set_statement_category(&self, category: impl Into<String>) {
        *self.statement_category.write() = category.into();
    }

    /// 返回 `ResultSet` 结构化事件分类。
    #[must_use]
    pub fn result_set_category(&self) -> String {
        self.result_set_category.read().clone()
    }

    /// 设置 `ResultSet` 结构化事件分类。
    pub fn set_result_set_category(&self, category: impl Into<String>) {
        *self.result_set_category.write() = category.into();
    }

    /// 是否记录 `DataSource`。
    #[must_use]
    pub fn is_data_source_log_enabled(&self) -> bool {
        Self::enabled(&self.data_source_log_enabled)
    }

    /// 设置是否记录 `DataSource`。
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

    /// 是否记录 `ResultSet`。
    #[must_use]
    pub fn is_result_set_log_enabled(&self) -> bool {
        Self::enabled(&self.result_set_log_enabled)
    }

    /// 设置是否记录 `ResultSet`。
    pub fn set_result_set_log_enabled(&self, enabled: bool) {
        self.result_set_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 `ResultSet` 错误。
    #[must_use]
    pub fn is_result_set_log_error_enabled(&self) -> bool {
        Self::enabled(&self.result_set_log_error_enabled)
    }

    /// 设置 `ResultSet` 错误日志开关。
    pub fn set_result_set_log_error_enabled(&self, enabled: bool) {
        self.result_set_log_error_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 `ResultSet` open。
    #[must_use]
    pub fn is_result_set_open_after_log_enabled(&self) -> bool {
        self.is_result_set_log_enabled() && Self::enabled(&self.result_set_open_after_log_enabled)
    }

    /// 设置 `ResultSet` open 日志开关。
    pub fn set_result_set_open_after_log_enabled(&self, enabled: bool) {
        self.result_set_open_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 `ResultSet` next。
    #[must_use]
    pub fn is_result_set_next_after_log_enabled(&self) -> bool {
        self.is_result_set_log_enabled() && Self::enabled(&self.result_set_next_after_log_enabled)
    }

    /// 设置 `ResultSet` next 日志开关。
    pub fn set_result_set_next_after_log_enabled(&self, enabled: bool) {
        self.result_set_next_after_log_enabled
            .store(enabled, Ordering::Release);
    }

    /// 是否记录 `ResultSet` close。
    #[must_use]
    pub fn is_result_set_close_after_log_enabled(&self) -> bool {
        self.is_result_set_log_enabled() && Self::enabled(&self.result_set_close_after_log_enabled)
    }

    /// 设置 `ResultSet` close 日志开关。
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
}
