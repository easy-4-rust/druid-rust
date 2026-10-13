use serde::de::DeserializeOwned;

use super::{HttpClient, HttpError};

/// 下游 Druid JSON 请求工具。
///
/// 对应 Java: `com.alibaba.druid.admin.util.HttpUtil`。
pub struct HttpUtil;

impl HttpUtil {
    /// GET 指定 URL 并反序列化为目标 DTO。
    ///
    /// 与 Java 版相比，Rust 不吞掉网络与反序列化失败，而是通过
    /// `Result` 保留错误边界，调用方再执行“部分节点失败可继续”的聚合策略。
    pub async fn get<T: DeserializeOwned>(
        client: &dyn HttpClient,
        url: &str,
    ) -> Result<T, HttpError> {
        // 下游 URL 可能含 userinfo 或 query 凭据；日志仅记录固定事件。
        tracing::info!("collecting druid admin endpoint");
        let value = client.get_json(url).await?;
        serde_json::from_value(value).map_err(|source| HttpError::Json {
            url: url.to_owned(),
            source,
        })
    }
}
