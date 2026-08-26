//! Rust 专用的 standard extensions。

mod character_reader;
mod character_writer;
mod connection_capabilities;
mod xml_result;
mod xml_source;

pub use crate::statement::ExecResult;
pub use character_reader::CharacterReader;
pub use character_writer::CharacterWriter;
pub use connection_capabilities::ConnectionCapabilities;
pub use xml_result::XmlResult;
pub use xml_source::XmlSource;
