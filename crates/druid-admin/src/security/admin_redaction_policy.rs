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
                        .filter(u8::is_ascii_alphanumeric)
                        .map(|byte| byte.to_ascii_lowercase() as char)
                        .collect::<String>();
                    // 属性映射可嵌套任意深度；凭据键不能依赖固定 DTO 字段名。
                    let credential = normalized.ends_with("password")
                        || normalized.ends_with("pwd")
                        || normalized.ends_with("secret")
                        || normalized.ends_with("token")
                        || matches!(
                            normalized.as_str(),
                            "apikey" | "accesskey" | "secretkey" | "authorization"
                        );
                    let diagnostic = normalized.ends_with("errormessage")
                        || normalized.ends_with("exceptionmessage")
                        || normalized.ends_with("errorstacktrace");
                    let connection = normalized.ends_with("url")
                        || normalized.ends_with("connectionstring")
                        || normalized.ends_with("username")
                        || matches!(normalized.as_str(), "dsn" | "user");
                    let query = matches!(
                        normalized.as_str(),
                        "sql"
                            | "formattedsql"
                            | "rawsql"
                            | "statementsql"
                            | "querysql"
                            | "lastslowparameters"
                            | "bindparameters"
                            | "sqlparameters"
                    ) || normalized.ends_with("sqltext");
                    if credential || diagnostic || connection || query || normalized == "lasterror"
                    {
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

#[cfg(test)]
mod tests {
    use super::AdminRedactionPolicy;
    use serde_json::json;

    #[test]
    fn secure_policy_redacts_nested_credentials_without_hiding_counts() {
        let mut response = json!({
            "Content": {
                "connectionProperties": {
                    "password": "database-secret",
                    "apiToken": "api-secret",
                    "clientSecret": "client-secret",
                    "accessKey": "access-secret",
                    "activeCount": 3
                }
            }
        });

        AdminRedactionPolicy::secure().redact_json(&mut response);

        let properties = &response["Content"]["connectionProperties"];
        for field in ["password", "apiToken", "clientSecret", "accessKey"] {
            assert_eq!(properties[field], "[REDACTED]", "{field} must be hidden");
        }
        assert_eq!(properties["activeCount"], 3);
    }

    #[test]
    fn secure_policy_redacts_sensitive_aliases_in_nested_arrays() {
        let original = json!({
            "Content": [{
                "databaseUrl": "jdbc:postgresql://host/db?password=secret",
                "connection_string": "Server=host;Password=secret",
                "dbUserName": "operator",
                "statementSql": "SELECT secret FROM users",
                "bindParameters": ["secret"],
                "exceptionMessage": "password=secret",
                "activeCount": 3,
                "nested": {"data_source_url": "jdbc:mysql://host/db?password=secret", "errorCount": 2}
            }]
        });
        let mut secure = original.clone();
        AdminRedactionPolicy::secure().redact_json(&mut secure);

        let content = &secure["Content"][0];
        for field in [
            "databaseUrl",
            "connection_string",
            "dbUserName",
            "statementSql",
            "bindParameters",
            "exceptionMessage",
        ] {
            assert_eq!(content[field], "[REDACTED]", "{field} must be hidden");
        }
        assert_eq!(content["nested"]["data_source_url"], "[REDACTED]");
        assert_eq!(content["activeCount"], 3);
        assert_eq!(content["nested"]["errorCount"], 2);

        let mut compatible = original.clone();
        AdminRedactionPolicy::compatibility().redact_json(&mut compatible);
        assert_eq!(compatible, original);
    }
}
