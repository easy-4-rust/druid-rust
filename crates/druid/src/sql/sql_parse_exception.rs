//! Deprecated SQLParseException 兼容对象。

use std::error::Error;
use std::fmt;

/// 旧 SQL parse 异常兼容对象。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.SQLParseException`。Java 类为空且
/// 仅有隐式无参构造器；新的扫描/解析代码应返回 `ParserException`。
#[deprecated(note = "对应 Java deprecated SQLParseException；请使用 ParserException")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SqlParseException;

#[allow(deprecated)]
impl fmt::Display for SqlParseException {
    fn fmt(&self, _formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        Ok(())
    }
}

#[allow(deprecated)]
impl Error for SqlParseException {}
