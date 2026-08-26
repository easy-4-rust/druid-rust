use crate::error::RdbcError;
use std::fmt;

/// Lossless UTF-16 reader supplied by a database adapter.
pub trait CharacterReader: fmt::Debug + Send {
    /// Reads UTF-16 code units and advances the cursor; zero denotes end of stream.
    fn read_utf16(&mut self, buffer: &mut [u16]) -> Result<usize, RdbcError>;

    /// Closes the underlying reader.
    fn close(&mut self) -> Result<(), RdbcError>;
}
