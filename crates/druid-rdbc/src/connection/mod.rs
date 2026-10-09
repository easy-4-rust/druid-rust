//! JDBC-like 连接标准。

mod client_info_status;

#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod connection;

pub use crate::types::Savepoint;
pub use client_info_status::ClientInfoStatus;
pub use connection::{Connection, TransactionIsolation};
