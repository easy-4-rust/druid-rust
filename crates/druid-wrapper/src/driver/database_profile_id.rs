use super::DatabaseProfileIdError;
use std::fmt;

/// 稳定的数据库产品档案标识；独立于 Druid `DbType` 方言枚举。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DatabaseProfileId(String);

impl DatabaseProfileId {
    /// 创建并校验档案标识。
    pub fn new(value: impl Into<String>) -> Result<Self, DatabaseProfileIdError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && value
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && value
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric);
        if valid {
            Ok(Self(value))
        } else {
            Err(DatabaseProfileIdError::new(value))
        }
    }

    /// 返回稳定字符串值。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DatabaseProfileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl AsRef<str> for DatabaseProfileId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
