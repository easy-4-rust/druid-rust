//! SQLx 直连驱动适配层。

/// SQLx 上的 bb8 外部池桥接。
pub mod bb8;
/// SQLx 上的 deadpool 外部池桥接。
pub mod deadpool;
pub mod sqlx_connection_adapter;
pub mod sqlx_connection_factory;
pub mod sqlx_database_meta_data;
pub mod sqlx_prepared_statement;

pub use sqlx_connection_adapter::SqlxConnectionAdapter;
pub use sqlx_connection_factory::SqlxConnectionFactory;
pub use sqlx_database_meta_data::SqlxDatabaseMetaData;
pub use sqlx_prepared_statement::SqlxPreparedStatement;
