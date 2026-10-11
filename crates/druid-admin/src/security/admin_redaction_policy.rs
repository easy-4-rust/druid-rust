use serde_json::Value;

/// 管理 JSON 的敏感字段脱敏策略。
///
/// Rust-only 安全对象；认证启用时由 MonitorViewServlet 在统一响应边界使用。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdminRedactionPolicy {
    enabled: bool,
}

impl AdminRedactionPolicy {
    /// 返回认证管理端的保守策略；隐藏 URL、用户名、原始 SQL、参数和驱动错误。
    #[must_use]
    pub const fn secure() -> Self {
        Self { enabled: true }
    }

    /// 返回本机无认证兼容模式的策略；保留原始管理 JSON。
    #[must_use]
    pub const fn compatibility() -> Self {
        Self { enabled: false }
    }

    /// 返回策略是否需要解析并脱敏 JSON 响应。
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        self.enabled
    }

    /// 原位脱敏一个 JSON 响应；参数为已解析值，返回值通过参数写回。
    pub fn redact_json(self, value: &mut Value) {
        if !self.enabled {
            return;
        }
        match value {
            Value::Array(values) => {
                for value in values {
                    self.redact_json(value);
                }
            }
            Value::Object(fields) => {
                for (key, field) in fields {
                    if field.is_null() {
                        continue;
                    }
                    let normalized = key
                        .bytes()
                        .filter(|byte| byte.is_ascii_alphanumeric())
                        .map(|byte| byte.to_ascii_lowercase() as char)
                        .collect::<String>();
                    if matches!(
                        normalized.as_str(),
                        "url"
                            | "jdbcurl"
                            | "connectionurl"
                            | "username"
                            | "sql"
                            | "formattedsql"
                            | "rawsql"
                            | "lastslowparameters"
                            | "bindparameters"
                            | "sqlparameters"
                            | "lasterrormessage"
                            | "lasterrorstacktrace"
                            | "lasterror"
                    ) {
                        *field = Value::String("[REDACTED]".to_owned());
                    } else {
                        self.redact_json(field);
                    }
                }
            }
            _ => {}
        }
    }
}
