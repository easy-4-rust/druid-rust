//! 对应 Java 类：com.alibaba.druid.pool.ExceptionSorter

/// 致命异常判断 trait。
pub trait ExceptionSorter: Send + Sync {
    fn is_exception_fatal(&self, error_code: i32, message: &str) -> bool;
}

/// 空实现。
pub struct NullExceptionSorter;
impl ExceptionSorter for NullExceptionSorter {
    fn is_exception_fatal(&self, _error_code: i32, _message: &str) -> bool {
        false
    }
}

/// PostgreSQL 致命异常判断。
pub struct PgExceptionSorter;
impl ExceptionSorter for PgExceptionSorter {
    fn is_exception_fatal(&self, error_code: i32, message: &str) -> bool {
        matches!(
            error_code,
            57_000 | 57_001 | 57_002 | 57_003 | 57_014 | 57_015
        ) || message.contains("connection has been closed")
            || message.contains("connection is not available")
    }
}

/// MySQL 致命异常判断。
pub struct MySqlExceptionSorter;
impl ExceptionSorter for MySqlExceptionSorter {
    fn is_exception_fatal(&self, error_code: i32, message: &str) -> bool {
        matches!(
            error_code,
            0 | 1053 | 1042 | 1043 | 1044 | 1045 | 1046 | 1047 | 1048 | 1049 | 1050 | 1051 | 1052
        ) || message.contains("Communications link failure")
            || message.contains("Connection refused")
    }
}
