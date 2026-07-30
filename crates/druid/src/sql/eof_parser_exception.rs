//! Parser 提前遇到 EOF 的异常。

use std::error::Error;
use std::fmt;
use std::ops::Deref;

use super::ParserException;

/// EOF parser 异常。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.EOFParserException`。
#[derive(Debug, Clone)]
pub struct EofParserException {
    parser_exception: ParserException,
}

impl EofParserException {
    /// 创建 message 固定为 `"EOF"` 的异常。
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser_exception: ParserException::with_message("EOF"),
        }
    }
}

impl Default for EofParserException {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for EofParserException {
    type Target = ParserException;

    fn deref(&self) -> &Self::Target {
        &self.parser_exception
    }
}

impl fmt::Display for EofParserException {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.parser_exception.fmt(formatter)
    }
}

impl Error for EofParserException {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.parser_exception.source()
    }
}
