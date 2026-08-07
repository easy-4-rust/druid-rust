use serde::Deserialize;

/// 产品档案创建未池化物理连接时采用的驱动运行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum DriverRuntimeMode {
    #[serde(rename = "sqlx")]
    Sqlx,
    #[serde(rename = "native")]
    Native,
    #[serde(rename = "jdbc_agent")]
    JdbcAgent,
    #[serde(rename = "http_sql")]
    HttpSql,
}
