use druid::core::{DruidError, SqlException};
use serde::Deserialize;

/// JDBC Agent 返回的结构化驱动异常。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentError {
    exception_class: String,
    message: String,
    #[serde(default)]
    sql_state: Option<String>,
    #[serde(default)]
    vendor_code: i32,
    #[serde(default, rename = "transientError")]
    transient_error: bool,
    #[serde(default)]
    recoverable: bool,
    #[serde(default)]
    fatal: bool,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    request_id: u64,
}

impl AgentError {
    /// 转换为 Druid 统一 SQL 异常，保留 JDBC 分类字段。
    #[must_use]
    pub fn into_druid_error(self) -> DruidError {
        let context = match self.session_id {
            Some(session_id) => format!(
                "{} [sessionId={session_id}, requestId={}]",
                self.message, self.request_id
            ),
            None => format!("{} [requestId={}]", self.message, self.request_id),
        };
        let mut exception = SqlException::new(self.vendor_code, self.sql_state, Some(context))
            .with_class_name(self.exception_class);
        if self.recoverable {
            exception = exception.recoverable();
        }
        let _classification = (self.transient_error, self.fatal);
        DruidError::SqlException(Box::new(exception))
    }
}
