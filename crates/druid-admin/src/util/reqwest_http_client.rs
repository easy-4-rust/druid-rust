use std::time::Duration;

use serde_json::Value;

use super::{HttpClient, HttpError};

/// 基于 Tokio/Reqwest 的生产 HTTP 客户端。
#[derive(Clone, Debug)]
pub struct ReqwestHttpClient {
    client: reqwest::Client,
}

impl ReqwestHttpClient {
    /// 创建带连接与总请求超时的客户端。
    ///
    /// # Errors
    ///
    /// TLS 或客户端配置构造失败时返回 `reqwest::Error`。
    pub fn new(
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, reqwest::Error> {
        reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map(|client| Self { client })
    }
}

#[async_trait::async_trait]
impl HttpClient for ReqwestHttpClient {
    async fn get_json(&self, url: &str) -> Result<Value, HttpError> {
        let response = self
            .client
            .get(url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .send()
            .await
            .map_err(|source| HttpError::Request {
                url: url.to_owned(),
                source,
            })?;
        let status = response.status();
        if status != reqwest::StatusCode::OK {
            return Err(HttpError::Status {
                url: url.to_owned(),
                status,
            });
        }
        response.json().await.map_err(|source| HttpError::Request {
            url: url.to_owned(),
            source,
        })
    }
}
