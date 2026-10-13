/// 管理端下游 HTTP 请求错误。
#[derive(thiserror::Error)]
pub enum HttpError {
    /// 网络或 TLS 失败。
    #[error("downstream request failed")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    /// 下游返回非 200 状态。
    #[error("downstream returned HTTP {status}")]
    Status {
        url: String,
        status: reqwest::StatusCode,
    },
    /// JSON 结构不符合目标 DTO。
    #[error("downstream response is invalid JSON")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },
}

impl std::fmt::Debug for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Request { .. } => formatter
                .debug_struct("Request")
                .field("url", &"[REDACTED]")
                .field("source", &"[REDACTED]")
                .finish(),
            Self::Status { status, .. } => formatter
                .debug_struct("Status")
                .field("url", &"[REDACTED]")
                .field("status", status)
                .finish(),
            Self::Json { .. } => formatter
                .debug_struct("Json")
                .field("url", &"[REDACTED]")
                .field("source", &"[REDACTED]")
                .finish(),
        }
    }
}
