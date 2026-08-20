//! 对应 Java：`com.alibaba.druid.filter.encoding.CharsetConvert`。

use crate::core::DruidError;
use encoding_rs::Encoding;

/// 客户端与服务端字符编码转换器。
///
/// Java 实现执行 `new String(s.getBytes(source), target)`，其中编码和解码都
/// 使用替换字符处理不可表示或非法序列。`encoding_rs` 在完整输入上的替换
/// 行为与该契约一致。
#[derive(Debug, Clone)]
pub struct CharsetConvert {
    client_encoding: Option<&'static Encoding>,
    server_encoding: Option<&'static Encoding>,
    enabled: bool,
}

impl CharsetConvert {
    /// 使用 Java charset 名称创建转换器。
    ///
    /// 任一名称为 `None` 时按 Java 关闭转换；非空但不支持的名称立即返回错误。
    pub fn new(
        client_encoding: Option<&str>,
        server_encoding: Option<&str>,
    ) -> Result<Self, DruidError> {
        let client = parse_encoding(client_encoding)?;
        let server = parse_encoding(server_encoding)?;
        let enabled = match (client_encoding, server_encoding) {
            (Some(client_name), Some(server_name)) => {
                !client_name.eq_ignore_ascii_case(server_name)
            }
            _ => false,
        };
        Ok(Self {
            client_encoding: client,
            server_encoding: server,
            enabled,
        })
    }

    /// 按客户端字节编码、服务端字符解码执行 Java `encode`。
    pub fn encode(&self, value: &str) -> Result<String, DruidError> {
        self.convert(value, self.client_encoding, self.server_encoding, "encode")
    }

    /// 按服务端字节编码、客户端字符解码执行 Java `decode`。
    pub fn decode(&self, value: &str) -> Result<String, DruidError> {
        self.convert(value, self.server_encoding, self.client_encoding, "decode")
    }

    /// 返回客户端编码的 canonical WHATWG 名称。
    #[must_use]
    pub fn client_encoding(&self) -> Option<&'static str> {
        self.client_encoding.map(Encoding::name)
    }

    /// 返回服务端编码的 canonical WHATWG 名称。
    #[must_use]
    pub fn server_encoding(&self) -> Option<&'static str> {
        self.server_encoding.map(Encoding::name)
    }

    /// 返回两端编码是否都存在且名称不同。
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn convert(
        &self,
        value: &str,
        source: Option<&'static Encoding>,
        target: Option<&'static Encoding>,
        operation: &str,
    ) -> Result<String, DruidError> {
        if !self.enabled || value.is_empty() {
            return Ok(value.to_owned());
        }
        let source = source.ok_or_else(|| {
            DruidError::InvalidArgument(format!("{operation} source encoding is missing"))
        })?;
        let target = target.ok_or_else(|| {
            DruidError::InvalidArgument(format!("{operation} target encoding is missing"))
        })?;
        let (bytes, _, _) = source.encode(value);
        let (decoded, _, _) = target.decode(bytes.as_ref());
        Ok(decoded.into_owned())
    }
}

fn parse_encoding(encoding: Option<&str>) -> Result<Option<&'static Encoding>, DruidError> {
    encoding
        .map(|name| {
            Encoding::for_label(name.as_bytes())
                .ok_or_else(|| DruidError::InvalidArgument(format!("unsupported charset `{name}`")))
        })
        .transpose()
}
