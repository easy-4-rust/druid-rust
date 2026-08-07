use super::{SqlInput, SqlOutput};
use crate::core::DruidError;

/// Custom mapping protocol from an SQL user-defined type to a Rust object.
///
/// Corresponds to Java: `java.sql.SQLData`. A connection type map associates a fully qualified
/// UDT name with an implementation. Drivers call `read_sql` and `write_sql`; attributes must
/// follow the SQL type declaration order.
pub trait SqlData: Send {
    /// Returns the fully qualified SQL name of the mapped UDT.
    fn sql_type_name(&self) -> &str;
    /// Reads attributes from `input` in declaration order and updates this object.
    ///
    /// `type_name` is supplied by the driver. Missing attributes, conversion failures, and
    /// database errors are returned. Corresponds to Java: `readSQL(SQLInput, String)`.
    fn read_sql(&mut self, input: &mut SqlInput, type_name: &str) -> Result<(), DruidError>;
    /// Writes object attributes to `output` in declaration order.
    ///
    /// Serialization or type-mapping failure returns an error. Corresponds to Java:
    /// `writeSQL(SQLOutput)`.
    fn write_sql(&self, output: &mut SqlOutput) -> Result<(), DruidError>;
}
