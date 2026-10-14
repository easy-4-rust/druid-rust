//! 产品档案连接 URL 的标准解析与诊断。

use crate::error::RdbcError;
use std::collections::HashMap;

/// 统一 RDBC 产品连接 URL。
///
/// 对应 Rust 产品扩展合同：选择数据库产品档案，并将 endpoint、database 与连接属性
/// 交给上层 runtime 解析。用户名、密码仅可用于驱动建连，诊断输出必须使用 `redacted`。
#[derive(Clone, PartialEq, Eq)]
pub struct RdbcConnectionUrl {
    raw: String,
    profile: String,
    endpoint: String,
    database: String,
    properties: HashMap<String, String>,
    subprotocol_style: bool,
}

impl RdbcConnectionUrl {
    /// 解析并校验 `rdbc:` 产品连接 URL；参数为完整 URL，返回标准解析值或无效参数错误。
    ///
    /// 支持 `rdbc:<profile>://<endpoint>` 与兼容形式
    /// `rdbc://<profile>/<endpoint>`。查询参数执行 URL 解码，重复键保留最后一个值。
    pub fn parse(value: &str) -> Result<Self, RdbcError> {
        let (parsed, profile, subprotocol_style) = if let Some(rest) = value.strip_prefix("rdbc:") {
            if rest.starts_with("//") {
                let parsed = url::Url::parse(value)
                    .map_err(|error| invalid_argument(format!("invalid RDBC URL: {error}")))?;
                let profile = parsed
                    .host_str()
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| invalid_argument("RDBC URL must contain a database profile"))?
                    .to_owned();
                (parsed, profile, false)
            } else {
                let (profile, target) = rest.split_once("://").ok_or_else(|| {
                    invalid_argument("RDBC URL must use 'rdbc:<profile>://<endpoint>'")
                })?;
                if profile.is_empty() {
                    return Err(invalid_argument("RDBC URL must contain a database profile"));
                }
                let parsed = url::Url::parse(&format!("rdbc://{target}"))
                    .map_err(|error| invalid_argument(format!("invalid RDBC URL: {error}")))?;
                (parsed, profile.to_owned(), true)
            }
        } else {
            let parsed = url::Url::parse(value)
                .map_err(|error| invalid_argument(format!("invalid RDBC URL: {error}")))?;
            return if parsed.scheme() == "rdbc" {
                Err(invalid_argument(
                    "RDBC URL must use 'rdbc:<profile>://<endpoint>'",
                ))
            } else {
                Err(invalid_argument("RDBC URL scheme must be 'rdbc'"))
            };
        };
        if parsed.username() != "" || parsed.password().is_some() || parsed.fragment().is_some() {
            return Err(invalid_argument(
                "RDBC URL forbids user-info and fragments; use connection properties",
            ));
        }
        let (endpoint, database) = if subprotocol_style {
            let host = parsed
                .host_str()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| invalid_argument("RDBC URL must contain an endpoint"))?;
            let host = if host.contains(':') {
                format!("[{host}]")
            } else {
                host.to_owned()
            };
            let endpoint = parsed
                .port()
                .map_or(host.clone(), |port| format!("{host}:{port}"));
            let database = parsed
                .path_segments()
                .ok_or_else(|| invalid_argument("RDBC URL path is invalid"))?
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("/");
            (endpoint, database)
        } else {
            let mut segments = parsed
                .path_segments()
                .ok_or_else(|| invalid_argument("RDBC URL path is invalid"))?
                .filter(|value| !value.is_empty());
            let endpoint = segments.next().unwrap_or_default().to_owned();
            let database = segments.collect::<Vec<_>>().join("/");
            (endpoint, database)
        };
        let properties = parsed
            .query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        Ok(Self {
            raw: value.to_owned(),
            profile,
            endpoint,
            database,
            properties,
            subprotocol_style,
        })
    }

    /// 返回原始 URL；该值可能含有凭据，只能用于连接解析。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// 返回 catalog 产品档案标识。
    #[must_use]
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// 返回 endpoint，通常为 `host[:port]` 或兼容形式的首个路径段。
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// 返回数据库、服务名或文件路径。
    #[must_use]
    pub fn database(&self) -> &str {
        &self.database
    }

    /// 返回 URL 解码后的连接属性；调用方不得记录其中的敏感值。
    #[must_use]
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// 按名称返回一个解码属性；参数为属性名，缺失时返回 `None`。
    #[must_use]
    pub fn property(&self, name: &str) -> Option<&str> {
        self.properties.get(name).map(String::as_str)
    }

    /// 返回移除查询参数后的诊断 URL，防止凭据进入日志。
    #[must_use]
    pub fn redacted(&self) -> String {
        let suffix = if self.database.is_empty() {
            String::new()
        } else {
            format!("/{}", self.database)
        };
        if self.subprotocol_style {
            format!("rdbc:{}://{}{suffix}", self.profile, self.endpoint)
        } else {
            format!("rdbc://{}/{}{suffix}", self.profile, self.endpoint)
        }
    }

    /// 使用 native scheme 构造无凭据驱动 URL；endpoint 为空时返回无效参数错误。
    pub fn network_url(&self, scheme: &str) -> Result<String, RdbcError> {
        if self.endpoint.is_empty() {
            return Err(invalid_argument(format!(
                "RDBC profile '{}' requires endpoint",
                self.profile
            )));
        }
        let suffix = if self.database.is_empty() {
            String::new()
        } else {
            format!("/{}", self.database)
        };
        Ok(format!("{scheme}://{}{suffix}", self.endpoint))
    }

    /// 构造供物理驱动使用的认证 URL；该返回值包含凭据，不得记录或用于诊断。
    pub fn authenticated_network_url(&self, scheme: &str) -> Result<String, RdbcError> {
        let mut parsed = url::Url::parse(&self.network_url(scheme)?)
            .map_err(|error| invalid_argument(format!("invalid target URL: {error}")))?;
        if let Some(username) = self.property("user") {
            parsed
                .set_username(username)
                .map_err(|()| invalid_argument("invalid RDBC username"))?;
        }
        if let Some(password) = self.property("password") {
            parsed
                .set_password(Some(password))
                .map_err(|()| invalid_argument("invalid RDBC password"))?;
        }
        Ok(parsed.to_string())
    }
}

impl std::fmt::Debug for RdbcConnectionUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RdbcConnectionUrl")
            .field("url", &self.redacted())
            .field("property_names", &self.properties.keys())
            .finish_non_exhaustive()
    }
}

fn invalid_argument(message: impl Into<String>) -> RdbcError {
    RdbcError::InvalidArgument(message.into())
}
