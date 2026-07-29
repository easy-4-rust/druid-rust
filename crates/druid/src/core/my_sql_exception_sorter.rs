//! `MySQL` 致命连接异常分类。

use super::{ExceptionSorter, ExceptionSorterProperties, SqlException, SqlExceptionCause};

/// `MySQL` 异常分类器。
///
/// 对应 Java: `com.alibaba.druid.pool.vendor.MySqlExceptionSorter`。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MySqlExceptionSorter;

impl ExceptionSorter for MySqlExceptionSorter {
    fn is_exception_fatal(&self, exception: &SqlException) -> bool {
        if exception.is_recoverable()
            || exception
                .sql_state()
                .is_some_and(|sql_state| sql_state.starts_with("08"))
        {
            return true;
        }

        let error_code = exception.error_code();
        if matches!(
            error_code,
            1004 | 1005
                | 1015
                | 1021
                | 1023
                | 1037
                | 1038
                | 1040
                | 1041
                | 1042
                | 1043
                | 1045
                | 1047
                | 1081
                | 1129
                | 1130
                | 1142
                | 1227
                | 1290
        ) || (-9000..=-8000).contains(&error_code)
            || exception.class_name().ends_with(".CommunicationsException")
        {
            return true;
        }

        if let Some(message) = exception.message() {
            const STREAMING_PREFIX: &str = "Streaming result set com.mysql.jdbc.RowDataDynamic";
            const STREAMING_SUFFIX: &str = "is still active. No statements may be issued when any streaming result sets are open and in use on a given connection. Ensure that you have called .close() on any active streaming result sets before attempting more queries.";
            if message.starts_with(STREAMING_PREFIX) && message.ends_with(STREAMING_SUFFIX) {
                return true;
            }

            let error_text = message.to_uppercase();
            if (error_code == 0 && error_text.contains("COMMUNICATIONS LINK FAILURE"))
                || error_text.contains("COULD NOT CREATE CONNECTION")
                || error_text.contains("NO DATASOURCE")
                || error_text.contains("NO ALIVE DATASOURCE")
            {
                return true;
            }
        }

        exception.causes().iter().take(5).any(|cause| match cause {
            SqlExceptionCause::SocketTimeout => true,
            SqlExceptionCause::ClassName(class_name) => {
                class_name.ends_with(".CommunicationsException")
            }
        })
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}
