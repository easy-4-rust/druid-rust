use druid::core::{DruidError, SqlException, SqlExceptionCause};
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
    #[serde(default)]
    assignable_types: Vec<String>,
    #[serde(default)]
    causes: Vec<String>,
    #[serde(default)]
    next_exceptions: Vec<Self>,
    #[serde(default)]
    update_counts: Option<Vec<i32>>,
}

impl AgentError {
    /// 转换为 Druid 统一 SQL 异常，保留 JDBC 分类字段。
    #[must_use]
    pub fn into_druid_error(self) -> DruidError {
        let update_counts = self.update_counts.clone();
        let exception = self.into_sql_exception(true);
        let cause = DruidError::SqlException(Box::new(exception));
        update_counts.map_or(cause.clone(), |update_counts| {
            DruidError::BatchUpdateException {
                update_counts,
                cause: Box::new(cause),
            }
        })
    }

    fn into_sql_exception(self, include_request_context: bool) -> SqlException {
        let context = match (include_request_context, self.session_id) {
            (true, Some(session_id)) => format!(
                "{} [sessionId={session_id}, requestId={}]",
                self.message, self.request_id
            ),
            (true, None) => format!("{} [requestId={}]", self.message, self.request_id),
            (false, _) => self.message,
        };
        let sql_state = if self.fatal && self.sql_state.is_none() {
            Some("08006".to_owned())
        } else {
            self.sql_state
        };
        let mut exception = SqlException::new(self.vendor_code, sql_state, Some(context))
            .with_class_name(self.exception_class);
        for assignable_type in self.assignable_types {
            exception = exception.with_assignable_type(assignable_type);
        }
        for cause in self.causes {
            exception = exception.with_cause(if cause == "java.net.SocketTimeoutException" {
                SqlExceptionCause::SocketTimeout
            } else {
                SqlExceptionCause::ClassName(cause)
            });
        }
        if self.recoverable || self.fatal {
            exception = exception.recoverable();
        }
        if self.transient_error {
            exception = exception.with_assignable_type("java.sql.SQLTransientException");
        }
        for next in self.next_exceptions {
            exception.set_next_exception(next.into_sql_exception(false));
        }
        exception
    }
}

#[cfg(test)]
mod tests {
    use super::AgentError;
    use druid::core::{DruidError, SqlExceptionCause};
    use serde_json::json;

    #[test]
    fn reconstructs_java_exception_hierarchy_cause_and_next_chain() {
        let error: AgentError = serde_json::from_value(json!({
            "exceptionClass": "java.sql.SQLTransientConnectionException",
            "message": "connection timed out",
            "sqlState": "08006",
            "vendorCode": 77,
            "transientError": true,
            "recoverable": false,
            "fatal": true,
            "sessionId": "session-1",
            "requestId": 42,
            "assignableTypes": [
                "java.sql.SQLTransientConnectionException",
                "java.sql.SQLTransientException",
                "java.sql.SQLException"
            ],
            "causes": ["java.net.SocketTimeoutException"],
            "nextExceptions": [{
                "exceptionClass": "java.sql.SQLIntegrityConstraintViolationException",
                "message": "duplicate key",
                "sqlState": "23505",
                "vendorCode": 88,
                "transientError": false,
                "recoverable": false,
                "fatal": false,
                "sessionId": "session-1",
                "requestId": 42,
                "assignableTypes": ["java.sql.SQLException"],
                "causes": [],
                "nextExceptions": [],
                "updateCounts": null
            }],
            "updateCounts": null
        }))
        .expect("the structured Agent error must deserialize");

        let DruidError::SqlException(exception) = error.into_druid_error() else {
            panic!("expected SqlException");
        };
        assert!(exception.is_instance_of("java.sql.SQLTransientException"));
        assert!(exception.is_recoverable());
        assert_eq!(exception.causes(), [SqlExceptionCause::SocketTimeout]);
        let next = exception
            .next_exception()
            .expect("next exception must survive");
        assert_eq!(next.sql_state(), Some("23505"));
        assert_eq!(next.error_code(), 88);
    }
}
