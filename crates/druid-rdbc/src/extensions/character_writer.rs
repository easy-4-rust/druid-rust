use crate::error::RdbcError;
use std::fmt;

/// Lossless UTF-16 writer supplied by a database adapter.
pub trait CharacterWriter: fmt::Debug + Send {
    /// Writes UTF-16 code units.
    fn write_utf16(&mut self, code_units: &[u16]) -> Result<usize, RdbcError>;

    /// Flushes the underlying writer.
    fn flush(&mut self) -> Result<(), RdbcError>;

    /// Closes the underlying writer.
    fn close(&mut self) -> Result<(), RdbcError>;
}
