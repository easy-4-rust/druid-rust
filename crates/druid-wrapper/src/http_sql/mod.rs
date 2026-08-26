//! 基于 HTTP 的 SQL 产品物理连接适配层。

mod http_sql_connection_adapter;
mod http_sql_connection_factory;
mod http_sql_provider;

pub use http_sql_connection_adapter::HttpSqlConnectionAdapter;
pub use http_sql_connection_factory::HttpSqlConnectionFactory;
pub use http_sql_provider::HttpSqlProvider;
