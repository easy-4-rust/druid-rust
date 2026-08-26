//! DuckDB 原生未池化驱动适配层。

mod duckdb_connection_adapter;
mod duckdb_connection_factory;

pub use duckdb_connection_adapter::DuckDbConnectionAdapter;
pub use duckdb_connection_factory::DuckDbConnectionFactory;
