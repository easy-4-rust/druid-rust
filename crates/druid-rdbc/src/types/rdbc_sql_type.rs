/// Identifies either a standard SQL type or a vendor-specific SQL type.
///
/// Corresponds to Java: `java.sql.SQLType`. Name, vendor, and type number jointly identify a
/// type for RDBC 4.2 binding and OUT-parameter registration; display names alone are insufficient.
pub trait SqlType {
    /// Returns the standard or vendor-defined type name.
    fn name(&self) -> &'static str;
    /// Returns the vendor identifier; standard RDBC types return `java.sql`.
    fn vendor(&self) -> &'static str;
    /// Returns the vendor type number; standard RDBC values match `java.sql.Types`.
    fn vendor_type_number(&self) -> i32;
}
