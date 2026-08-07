use serde::Deserialize;

/// 数据库连接协议族；产品档案可以共享协议但保留独立能力声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ProtocolFamily {
    #[serde(rename = "mysql")]
    MySql,
    #[serde(rename = "postgresql")]
    PostgreSql,
    #[serde(rename = "sqlite")]
    SQLite,
    #[serde(rename = "oracle")]
    Oracle,
    #[serde(rename = "sqlserver")]
    SqlServer,
    #[serde(rename = "embedded")]
    Embedded,
    #[serde(rename = "jdbc")]
    Jdbc,
    #[serde(rename = "http_sql")]
    HttpSql,
}

impl ProtocolFamily {
    /// 返回驱动标识和诊断使用的稳定协议族名称。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MySql => "mysql",
            Self::PostgreSql => "postgresql",
            Self::SQLite => "sqlite",
            Self::Oracle => "oracle",
            Self::SqlServer => "sqlserver",
            Self::Embedded => "embedded",
            Self::Jdbc => "jdbc",
            Self::HttpSql => "http_sql",
        }
    }
}
