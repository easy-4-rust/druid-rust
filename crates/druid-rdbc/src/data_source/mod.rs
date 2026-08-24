//! `javax.sql` 对齐的数据源标准。

mod common_data_source;
mod connection_pool_data_source;
#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod data_source;
mod pooled_connection;
mod xa_connection;
mod xa_data_source;

pub use common_data_source::CommonDataSource;
pub use connection_pool_data_source::ConnectionPoolDataSource;
pub use data_source::DataSource;
pub use pooled_connection::PooledConnection;
pub use xa_connection::XaConnection;
pub use xa_data_source::XaDataSource;
