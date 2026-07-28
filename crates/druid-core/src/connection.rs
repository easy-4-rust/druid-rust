//! `Connection` 兼容导出。
//!
//! 新代码必须使用 `PhysicalConnection`。本模块只为迁移中的调用方保留
//! 原路径，不定义第二套连接 SPI。

pub use crate::conn_state::ConnState;
pub use crate::connection_ext::ConnectionExt;
pub use crate::exec_result::ExecResult;
pub use crate::meta_data::MetaData;
pub use crate::physical_connection::PhysicalConnection;
pub use crate::physical_connection::PhysicalConnection as Connection;
pub use crate::row::Row;
pub use crate::savepoint::Savepoint;
pub use crate::statement_type::StatementType;
