//! Parser 禁止注释异常。

use std::error::Error;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

use super::ParserException;

/// 当前 parser 配置不允许注释。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.NotAllowCommentException`。
#[derive(Debug, Clone)]
pub struct NotAllowCommentException {
    parser_exception: ParserException,
}

impl NotAllowCommentException {
    /// 创建默认 `"comment not allow"` 异常。
    #[must_use]
    pub fn new() -> Self {
        Self::with_message("comment not allow")
    }

    /// 使用指定 message 创建异常。
    #[must_use]
    pub fn with_message(message: impl Into<String>) -> Self {
        Self {
            parser_exception: ParserException::with_message(message),
        }
    }

    /// 使用指定 message 与 cause 创建异常。
    #[must_use]
    pub fn with_message_and_cause(
        message: impl Into<String>,
        cause: Arc<dyn Error + Send + Sync>,
    ) -> Self {
        Self {
            parser_exception: ParserException::with_message_and_cause(message, cause),
        }
    }
}

impl Default for NotAllowCommentException {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for NotAllowCommentException {
    type Target = ParserException;

    fn deref(&self) -> &Self::Target {
        &self.parser_exception
    }
}

impl fmt::Display for NotAllowCommentException {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.parser_exception.fmt(formatter)
    }
}

impl Error for NotAllowCommentException {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.parser_exception.source()
    }
}
