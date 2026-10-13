use super::XaErrorCode;
use crate::error::RdbcError;
use std::fmt;

/// 保留 XA 协议错误码、诊断文本和可选标准错误来源的结构化错误。
/// 对应 Java: `javax.transaction.xa.XAException`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XaError {
    code: XaErrorCode,
    message: String,
    source: Option<Box<RdbcError>>,
}

impl XaError {
    /// 创建带原始 XA 错误码和消息的错误。
    pub fn new(code: XaErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
        }
    }

    /// 创建保留标准 RDBC 原因链的 XA 错误。
    pub fn with_source(code: XaErrorCode, message: impl Into<String>, source: RdbcError) -> Self {
        Self {
            code,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// 创建明确表示后端不支持 XA 的错误。
    #[must_use]
    pub fn feature_not_supported(operation: &'static str) -> Self {
        Self::with_source(
            XaErrorCode::XAER_RMERR,
            format!("XA operation is not supported: {operation}"),
            RdbcError::feature_not_supported(operation),
        )
    }

    /// 返回原始 XA 错误码。
    #[must_use]
    pub fn code(&self) -> XaErrorCode {
        self.code
    }

    /// 返回错误诊断消息。
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 返回可选的底层标准 RDBC 错误。
    #[must_use]
    pub fn source_error(&self) -> Option<&RdbcError> {
        self.source.as_deref()
    }
}

impl fmt::Display for XaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        std::write!(formatter, "XA error {}: {}", self.code.0, self.message)
    }
}

impl std::error::Error for XaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_deref().map(|source| source as _)
    }
}
