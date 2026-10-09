//! Rust 专用的 standard extensions。

mod buffered_row_set;
mod character_reader;
mod character_writer;
mod connection_capabilities;
mod shared_result_set;
mod shared_result_set_borrow;
mod shared_result_set_borrow_future;
mod shared_result_set_state;
mod xml_result;
mod xml_source;

pub use crate::statement::ExecResult;
pub use buffered_row_set::BufferedRowSet;
pub use character_reader::CharacterReader;
pub use character_writer::CharacterWriter;
pub use connection_capabilities::ConnectionCapabilities;
pub use shared_result_set::SharedResultSet;
pub use shared_result_set_borrow::SharedResultSetBorrow;
pub use xml_result::XmlResult;
pub use xml_source::XmlSource;
