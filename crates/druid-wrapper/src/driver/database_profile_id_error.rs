use std::fmt;

/// 数据库产品档案标识校验错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseProfileIdError {
    value: String,
}

impl DatabaseProfileIdError {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    /// 返回未通过校验的原始值。
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for DatabaseProfileIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid database profile id '{}': expected lowercase ASCII letters, digits, or '-'",
            self.value
        )
    }
}

impl std::error::Error for DatabaseProfileIdError {}
