//! 对应 Java：`com.alibaba.druid.filter.encoding.CharsetParameter`。

/// 已弃用的字符编码参数 Bean。
///
/// 保留 Java 配置键与可变 getter/setter 语义，供旧配置适配器使用。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[deprecated(note = "对应 Java CharsetParameter，后续应直接配置 EncodingConvertFilter")]
pub struct CharsetParameter {
    client_encoding: Option<String>,
    server_encoding: Option<String>,
}

#[allow(deprecated)]
impl CharsetParameter {
    /// Java `clientEncoding` 配置键。
    pub const CLIENT_ENCODING_KEY: &'static str = "clientEncoding";
    /// Java `serverEncoding` 配置键。
    pub const SERVER_ENCODING_KEY: &'static str = "serverEncoding";

    /// 返回客户端编码。
    #[must_use]
    pub fn client_encoding(&self) -> Option<&str> {
        self.client_encoding.as_deref()
    }

    /// 设置客户端编码。
    pub fn set_client_encoding(&mut self, client_encoding: Option<String>) {
        self.client_encoding = client_encoding;
    }

    /// 返回服务端编码。
    #[must_use]
    pub fn server_encoding(&self) -> Option<&str> {
        self.server_encoding.as_deref()
    }

    /// 设置服务端编码。
    pub fn set_server_encoding(&mut self, server_encoding: Option<String>) {
        self.server_encoding = server_encoding;
    }
}
