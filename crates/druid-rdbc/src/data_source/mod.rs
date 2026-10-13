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
mod pooled_connection_event_registry;
mod statement_event;
mod statement_event_listener;
mod xa_connection;
mod xa_data_source;
mod xa_error;
mod xa_error_code;
mod xa_flags;
mod xa_prepare_result;
mod xa_resource;
mod xid;

pub use common_data_source::CommonDataSource;
pub use connection_event::{ConnectionEvent, PooledConnectionSource};
pub use connection_event_listener::ConnectionEventListener;
pub use connection_pool_data_source::ConnectionPoolDataSource;
pub use data_source::DataSource;
pub use pooled_connection::PooledConnection;
pub use pooled_connection_event_registry::PooledConnectionEventRegistry;
pub use statement_event::{StatementEvent, StatementSource};
pub use statement_event_listener::StatementEventListener;
pub use xa_connection::XaConnection;
pub use xa_data_source::XaDataSource;
pub use xa_error::XaError;
pub use xa_error_code::XaErrorCode;
pub use xa_flags::{flags, XaFlags};
pub use xa_prepare_result::XaPrepareResult;
pub use xa_resource::XaResource;
pub use xid::{Xid, XID_MAX_BQUAL_LENGTH, XID_MAX_GTRID_LENGTH};
