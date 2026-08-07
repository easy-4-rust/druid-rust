//! DuckDB 原生未池化驱动适配层。

mod duckdb_connection_adapter;
mod duckdb_connection_factory;
mod duckdb_database_meta_data;
mod duckdb_prepared_statement;

pub use duckdb_connection_adapter::DuckDbConnectionAdapter;
pub use duckdb_connection_factory::DuckDbConnectionFactory;
pub use duckdb_database_meta_data::DuckDbDatabaseMetaData;
pub use duckdb_prepared_statement::DuckDbPreparedStatement;
