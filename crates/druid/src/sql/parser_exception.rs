//! Druid SQL parser 基础异常。

use std::error::Error;
use std::fmt;
use std::sync::Arc;

/// SQL parser 基础异常。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.ParserException`。Rust 不复制
/// `FastsqlException/RuntimeException` 继承树，但保留 nullable message、cause、
/// line、column 及五个构造路径。
#[derive(Clone)]
pub struct ParserException {
    message: Option<String>,
    line: i32,
    column: i32,
    cause: Option<Arc<dyn Error + Send + Sync>>,
}

impl ParserException {
    /// 创建无 message、无 cause、位置为 0:0 的异常。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            message: None,
            line: 0,
            column: 0,
            cause: None,
        }
    }

    /// 使用 message 创建异常，位置为 0:0。
    #[must_use]
    pub fn with_message(message: impl Into<String>) -> Self {
        Self {
            message: Some(message.into()),
            ..Self::new()
        }
    }

    /// 使用 message 与 cause 创建异常，位置为 0:0。
    #[must_use]
    pub fn with_message_and_cause(
        message: impl Into<String>,
        cause: Arc<dyn Error + Send + Sync>,
    ) -> Self {
        Self {
            message: Some(message.into()),
            line: 0,
            column: 0,
            cause: Some(cause),
        }
    }

    /// 使用 message、line、column 创建异常。
    #[must_use]
    pub fn with_position(message: impl Into<String>, line: i32, column: i32) -> Self {
        Self {
            message: Some(message.into()),
            line,
            column,
            cause: None,
        }
    }

    /// 包装另一个错误并附加 source SQL。
    ///
    /// 对应 Java：`ParserException(Throwable,String)` 的精确换行模板。
    #[must_use]
    pub fn from_cause_and_sql(cause: Arc<dyn Error + Send + Sync>, sql: impl AsRef<str>) -> Self {
        let message = format!(
            "parse error. detail message is :\n{cause}\nsource sql is : \n{}",
            sql.as_ref()
        );
        Self {
            message: Some(message),
            line: 0,
            column: 0,
            cause: Some(cause),
        }
    }

    /// 返回 nullable message。
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// 返回 Java 字段 line。
    #[must_use]
    pub const fn line(&self) -> i32 {
        self.line
    }

    /// 返回 Java 字段 column。
    #[must_use]
    pub const fn column(&self) -> i32 {
        self.column
    }
}

impl Default for ParserException {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ParserException {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message.as_deref().unwrap_or(""))
    }
}

impl fmt::Debug for ParserException {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParserException")
            .field("message", &self.message)
            .field("line", &self.line)
            .field("column", &self.column)
            .field("has_cause", &self.cause.is_some())
            .finish()
    }
}

impl Error for ParserException {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}
