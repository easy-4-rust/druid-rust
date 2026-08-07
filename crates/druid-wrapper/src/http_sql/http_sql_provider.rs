/// HTTP SQL 产品协议族。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpSqlProvider {
    Rqlite,
    CloudflareD1,
}

impl HttpSqlProvider {
    /// 从驱动清单 provider ID 解析 HTTP SQL 协议。
    pub fn from_provider_id(provider_id: &str) -> Option<Self> {
        match provider_id {
            "rqlite-http" => Some(Self::Rqlite),
            "cloudflare-d1-http" => Some(Self::CloudflareD1),
            _ => None,
        }
    }

    /// 返回稳定 provider ID。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rqlite => "rqlite-http",
            Self::CloudflareD1 => "cloudflare-d1-http",
        }
    }
}
