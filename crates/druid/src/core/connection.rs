//! `Connection` 兼容导出。
//!
//! 新代码必须使用 `PhysicalConnection`。本模块只为迁移中的调用方保留
//! 原路径，不定义第二套连接 SPI。

pub use super::conn_state::ConnState;
pub use super::connection_ext::ConnectionExt;
pub use super::exec_result::ExecResult;
pub use super::meta_data::MetaData;
pub use super::physical_connection::PhysicalConnection;
pub use super::physical_connection::PhysicalConnection as Connection;
pub use super::row::Row;
pub use super::savepoint::Savepoint;
pub use super::statement_type::StatementType;
