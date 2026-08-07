//! 基于 HTTP 的 SQL 产品物理连接适配层。

mod http_sql_connection_adapter;
mod http_sql_connection_factory;
mod http_sql_database_meta_data;
mod http_sql_prepared_statement;
mod http_sql_provider;

pub use http_sql_connection_adapter::HttpSqlConnectionAdapter;
pub use http_sql_connection_factory::HttpSqlConnectionFactory;
pub use http_sql_database_meta_data::HttpSqlDatabaseMetaData;
pub use http_sql_prepared_statement::HttpSqlPreparedStatement;
pub use http_sql_provider::HttpSqlProvider;
