//! JDBC-like 连接标准。

#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod connection;

pub use crate::types::Savepoint;
pub use connection::Connection;
