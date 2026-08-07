/// Integer constants identifying generic RDBC SQL types.
///
/// Corresponds to Java: `java.sql.Types`. Numbers retain the Java 8 values used for parameter
/// registration, binding, and metadata. Vendor-specific types use `OTHER` or a vendor number.
pub struct Types;

impl Types {
    /// SQL `BIT`。
    pub const BIT: i32 = -7;
    /// SQL `TINYINT`。
    pub const TINYINT: i32 = -6;
    /// SQL `SMALLINT`。
    pub const SMALLINT: i32 = 5;
    /// SQL `INTEGER`。
    pub const INTEGER: i32 = 4;
    /// SQL `BIGINT`。
    pub const BIGINT: i32 = -5;
    /// SQL `FLOAT`。
    pub const FLOAT: i32 = 6;
    /// SQL `REAL`。
    pub const REAL: i32 = 7;
    /// SQL `DOUBLE`。
    pub const DOUBLE: i32 = 8;
    /// SQL `NUMERIC`。
    pub const NUMERIC: i32 = 2;
    /// SQL `DECIMAL`。
    pub const DECIMAL: i32 = 3;
    /// SQL fixed-length character `CHAR`.
    pub const CHAR: i32 = 1;
    /// SQL variable-length character `VARCHAR`.
    pub const VARCHAR: i32 = 12;
    /// SQL long variable-length character `LONGVARCHAR`.
    pub const LONGVARCHAR: i32 = -1;
    /// SQL date `DATE`.
    pub const DATE: i32 = 91;
    /// SQL time `TIME`.
    pub const TIME: i32 = 92;
    /// SQL timestamp `TIMESTAMP`.
    pub const TIMESTAMP: i32 = 93;
    /// SQL fixed-length binary `BINARY`.
    pub const BINARY: i32 = -2;
    /// SQL variable-length binary `VARBINARY`.
    pub const VARBINARY: i32 = -3;
    /// SQL long variable-length binary `LONGVARBINARY`.
    pub const LONGVARBINARY: i32 = -4;
    /// SQL `NULL` type.
    pub const NULL: i32 = 0;
    /// SQL type recognized by the driver but outside standard mappings.
    pub const OTHER: i32 = 1111;
    /// Java/Rust object-mapped type.
    pub const JAVA_OBJECT: i32 = 2000;
    /// SQL user-defined `DISTINCT` type.
    pub const DISTINCT: i32 = 2001;
    /// SQL structured `STRUCT` type.
    pub const STRUCT: i32 = 2002;
    /// SQL `ARRAY`。
    pub const ARRAY: i32 = 2003;
    /// SQL binary large object `BLOB`.
    pub const BLOB: i32 = 2004;
    /// SQL character large object `CLOB`.
    pub const CLOB: i32 = 2005;
    /// SQL structured-type reference `REF`.
    pub const REF: i32 = 2006;
    /// SQL external data link `DATALINK`.
    pub const DATALINK: i32 = 70;
    /// SQL `BOOLEAN`。
    pub const BOOLEAN: i32 = 16;
    /// SQL row identifier `ROWID`.
    pub const ROWID: i32 = -8;
    /// SQL national-character fixed-length `NCHAR`.
    pub const NCHAR: i32 = -15;
    /// SQL national-character variable-length `NVARCHAR`.
    pub const NVARCHAR: i32 = -9;
    /// SQL national-character long variable-length `LONGNVARCHAR`.
    pub const LONGNVARCHAR: i32 = -16;
    /// SQL national-character large object `NCLOB`.
    pub const NCLOB: i32 = 2011;
    /// SQL XML value.
    pub const SQLXML: i32 = 2009;
    /// RDBC 4.2 SQL `REF CURSOR`。
    pub const REF_CURSOR: i32 = 2012;
    /// RDBC 4.2 SQL `TIME` with time zone.
    pub const TIME_WITH_TIMEZONE: i32 = 2013;
    /// RDBC 4.2 SQL `TIMESTAMP` with time zone.
    pub const TIMESTAMP_WITH_TIMEZONE: i32 = 2014;
}
