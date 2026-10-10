//! `javax.sql` 对齐的数据源标准。

mod common_data_source;
mod connection_event;
mod connection_event_listener;
mod connection_pool_data_source;
#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod data_source;
mod pooled_connection;
mod statement_event;
mod statement_event_listener;
mod xa_connection;
mod xa_data_source;

pub use common_data_source::CommonDataSource;
pub use connection_event::{ConnectionEvent, PooledConnectionSource};
pub use connection_event_listener::ConnectionEventListener;
pub use connection_pool_data_source::ConnectionPoolDataSource;
pub use data_source::DataSource;
pub use pooled_connection::PooledConnection;
pub use statement_event::{StatementEvent, StatementSource};
pub use statement_event_listener::StatementEventListener;
pub use xa_connection::XaConnection;
pub use xa_data_source::XaDataSource;
