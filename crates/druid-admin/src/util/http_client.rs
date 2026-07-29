use serde_json::Value;

use super::HttpError;

/// 管理端采集下游 Druid 节点的 HTTP SPI。
#[async_trait::async_trait]
pub trait HttpClient: Send + Sync {
    /// 以 GET 请求读取 JSON。
    async fn get_json(&self, url: &str) -> Result<Value, HttpError>;
}
