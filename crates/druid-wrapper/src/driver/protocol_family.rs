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
