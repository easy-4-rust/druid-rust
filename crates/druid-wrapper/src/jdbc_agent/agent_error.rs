use druid::core::{DruidError, SqlException};
use serde::Deserialize;

/// JDBC Agent 返回的结构化驱动异常。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentError {
    class_name: String,
    message: String,
    #[serde(default)]
    sql_state: Option<String>,
    #[serde(default)]
    error_code: i32,
    #[serde(default)]
    recoverable: bool,
}

impl AgentError {
    /// 转换为 Druid 统一 SQL 异常，保留 JDBC 分类字段。
    #[must_use]
    pub fn into_druid_error(self) -> DruidError {
        let mut exception = SqlException::new(self.error_code, self.sql_state, Some(self.message))
            .with_class_name(self.class_name);
        if self.recoverable {
            exception = exception.recoverable();
        }
        DruidError::SqlException(Box::new(exception))
    }
}
