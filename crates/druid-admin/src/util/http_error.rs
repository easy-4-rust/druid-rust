/// 管理端下游 HTTP 请求错误。
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    /// 网络或 TLS 失败。
    #[error("request {url} failed: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    /// 下游返回非 200 状态。
    #[error("request {url} returned HTTP {status}")]
    Status {
        url: String,
        status: reqwest::StatusCode,
    },
    /// JSON 结构不符合目标 DTO。
    #[error("response from {url} is invalid JSON: {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },
}
