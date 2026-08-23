//! `Connection` 兼容导出。
//!
//! 新代码必须使用 `PhysicalConnection`。本模块只为迁移中的调用方保留
//! 原路径，不定义第二套连接 SPI。

pub use super::conn_state::ConnState;
pub use super::connection_ext::ConnectionExt;
pub use super::meta_data::MetaData;
pub use super::row::Row;
pub use super::savepoint::Savepoint;
pub use super::statement_type::StatementType;
pub use druid_rdbc::spi::ExecResult;
pub use druid_rdbc::spi::PhysicalConnection;
pub use druid_rdbc::spi::PhysicalConnection as Connection;
