use crate::error::RdbcError;
use crate::types::SqlType;

/// Java 8 标准 SQL 类型枚举。对应 Java: `java.sql.JDBCType`。
///
/// 每种类型实现 `SqlType`，名称采用 Java 标准常量，供应方为 `java.sql`，
/// 类型编号与 `Types` 一致；保留 JDBC 4.2 的游标和含时区时间类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JdbcType {
    /// SQL BIT 类型。对应 Java: `JDBCType#BIT`。
    Bit,
    /// SQL TINYINT 类型。对应 Java: `JDBCType#TINYINT`。
    TinyInt,
    /// SQL SMALLINT 类型。对应 Java: `JDBCType#SMALLINT`。
    SmallInt,
    /// SQL INTEGER 类型。对应 Java: `JDBCType#INTEGER`。
    Integer,
    /// SQL BIGINT 类型。对应 Java: `JDBCType#BIGINT`。
    BigInt,
    /// SQL FLOAT 类型。对应 Java: `JDBCType#FLOAT`。
    Float,
    /// SQL REAL 类型。对应 Java: `JDBCType#REAL`。
    Real,
    /// SQL DOUBLE 类型。对应 Java: `JDBCType#DOUBLE`。
    Double,
    /// SQL NUMERIC 类型。对应 Java: `JDBCType#NUMERIC`。
    Numeric,
    /// SQL DECIMAL 类型。对应 Java: `JDBCType#DECIMAL`。
    Decimal,
    /// SQL CHAR 类型。对应 Java: `JDBCType#CHAR`。
    Char,
    /// SQL VARCHAR 类型。对应 Java: `JDBCType#VARCHAR`。
    VarChar,
    /// SQL LONGVARCHAR 类型。对应 Java: `JDBCType#LONGVARCHAR`。
    LongVarChar,
    /// SQL DATE 类型。对应 Java: `JDBCType#DATE`。
    Date,
    /// SQL TIME 类型。对应 Java: `JDBCType#TIME`。
    Time,
    /// SQL TIMESTAMP 类型。对应 Java: `JDBCType#TIMESTAMP`。
    Timestamp,
    /// SQL BINARY 类型。对应 Java: `JDBCType#BINARY`。
    Binary,
    /// SQL VARBINARY 类型。对应 Java: `JDBCType#VARBINARY`。
    VarBinary,
    /// SQL LONGVARBINARY 类型。对应 Java: `JDBCType#LONGVARBINARY`。
    LongVarBinary,
    /// SQL NULL 类型。对应 Java: `JDBCType#NULL`。
    Null,
    /// SQL OTHER 类型。对应 Java: `JDBCType#OTHER`。
    Other,
    /// SQL JAVA_OBJECT 类型。对应 Java: `JDBCType#JAVA_OBJECT`。
    JavaObject,
    /// SQL DISTINCT 类型。对应 Java: `JDBCType#DISTINCT`。
    Distinct,
    /// SQL STRUCT 类型。对应 Java: `JDBCType#STRUCT`。
    Struct,
    /// SQL ARRAY 类型。对应 Java: `JDBCType#ARRAY`。
    Array,
    /// SQL BLOB 类型。对应 Java: `JDBCType#BLOB`。
    Blob,
    /// SQL CLOB 类型。对应 Java: `JDBCType#CLOB`。
    Clob,
    /// SQL REF 类型。对应 Java: `JDBCType#REF`。
    Ref,
    /// SQL DATALINK 类型。对应 Java: `JDBCType#DATALINK`。
    DataLink,
    /// SQL BOOLEAN 类型。对应 Java: `JDBCType#BOOLEAN`。
    Boolean,
    /// SQL ROWID 类型。对应 Java: `JDBCType#ROWID`。
    RowId,
    /// SQL NCHAR 类型。对应 Java: `JDBCType#NCHAR`。
    NChar,
    /// SQL NVARCHAR 类型。对应 Java: `JDBCType#NVARCHAR`。
    NVarChar,
    /// SQL LONGNVARCHAR 类型。对应 Java: `JDBCType#LONGNVARCHAR`。
    LongNVarChar,
    /// SQL NCLOB 类型。对应 Java: `JDBCType#NCLOB`。
    NClob,
    /// SQL SQLXML 类型。对应 Java: `JDBCType#SQLXML`。
    SqlXml,
    /// SQL REF_CURSOR 类型。对应 Java: `JDBCType#REF_CURSOR`。
    RefCursor,
    /// SQL TIME_WITH_TIMEZONE 类型。对应 Java: `JDBCType#TIME_WITH_TIMEZONE`。
    TimeWithTimezone,
    /// SQL TIMESTAMP_WITH_TIMEZONE 类型。对应 Java: `JDBCType#TIMESTAMP_WITH_TIMEZONE`。
    TimestampWithTimezone,
}

impl JdbcType {
    /// 按标准类型编号查找 SQL 类型。对应 Java: `JDBCType#valueOf(int)`。
    ///
    /// 参数 `type_number` 为 `Types` 中的编号；返回对应枚举，未知编号返回
    /// `RdbcError::InvalidArgument`，映射 Java 的 `IllegalArgumentException`。
    pub fn value_of(type_number: i32) -> Result<Self, RdbcError> {
        // 与 Java 遍历枚举值的查找语义一致，复用同一供应方编号映射。
        let standard_types = [
            Self::Bit,
            Self::TinyInt,
            Self::SmallInt,
            Self::Integer,
            Self::BigInt,
            Self::Float,
            Self::Real,
            Self::Double,
            Self::Numeric,
            Self::Decimal,
            Self::Char,
            Self::VarChar,
            Self::LongVarChar,
            Self::Date,
            Self::Time,
            Self::Timestamp,
            Self::Binary,
            Self::VarBinary,
            Self::LongVarBinary,
            Self::Null,
            Self::Other,
            Self::JavaObject,
            Self::Distinct,
            Self::Struct,
            Self::Array,
            Self::Blob,
            Self::Clob,
            Self::Ref,
            Self::DataLink,
            Self::Boolean,
            Self::RowId,
            Self::NChar,
            Self::NVarChar,
            Self::LongNVarChar,
            Self::NClob,
            Self::SqlXml,
            Self::RefCursor,
            Self::TimeWithTimezone,
            Self::TimestampWithTimezone,
        ];
        standard_types
            .into_iter()
            .find(|value| value.vendor_type_number() == type_number)
            .ok_or_else(|| RdbcError::InvalidArgument(format!("未知 JDBC 类型编号：{type_number}")))
    }
}

impl SqlType for JdbcType {
    /// 返回标准类型名称。对应 Java: `JDBCType#getName`。
    fn name(&self) -> &'static str {
        match self {
            Self::Bit => "BIT",
            Self::TinyInt => "TINYINT",
            Self::SmallInt => "SMALLINT",
            Self::Integer => "INTEGER",
            Self::BigInt => "BIGINT",
            Self::Float => "FLOAT",
            Self::Real => "REAL",
            Self::Double => "DOUBLE",
            Self::Numeric => "NUMERIC",
            Self::Decimal => "DECIMAL",
            Self::Char => "CHAR",
            Self::VarChar => "VARCHAR",
            Self::LongVarChar => "LONGVARCHAR",
            Self::Date => "DATE",
            Self::Time => "TIME",
            Self::Timestamp => "TIMESTAMP",
            Self::Binary => "BINARY",
            Self::VarBinary => "VARBINARY",
            Self::LongVarBinary => "LONGVARBINARY",
            Self::Null => "NULL",
            Self::Other => "OTHER",
            Self::JavaObject => "JAVA_OBJECT",
            Self::Distinct => "DISTINCT",
            Self::Struct => "STRUCT",
            Self::Array => "ARRAY",
            Self::Blob => "BLOB",
            Self::Clob => "CLOB",
            Self::Ref => "REF",
            Self::DataLink => "DATALINK",
            Self::Boolean => "BOOLEAN",
            Self::RowId => "ROWID",
            Self::NChar => "NCHAR",
            Self::NVarChar => "NVARCHAR",
            Self::LongNVarChar => "LONGNVARCHAR",
            Self::NClob => "NCLOB",
            Self::SqlXml => "SQLXML",
            Self::RefCursor => "REF_CURSOR",
            Self::TimeWithTimezone => "TIME_WITH_TIMEZONE",
            Self::TimestampWithTimezone => "TIMESTAMP_WITH_TIMEZONE",
        }
    }

    /// 返回类型供应方 `java.sql`。对应 Java: `JDBCType#getVendor`。
    fn vendor(&self) -> &'static str {
        "java.sql"
    }

    /// 返回供应方类型编号。对应 Java: `JDBCType#getVendorTypeNumber`。
    fn vendor_type_number(&self) -> i32 {
        match self {
            Self::Bit => -7,
            Self::TinyInt => -6,
            Self::SmallInt => 5,
            Self::Integer => 4,
            Self::BigInt => -5,
            Self::Float => 6,
            Self::Real => 7,
            Self::Double => 8,
            Self::Numeric => 2,
            Self::Decimal => 3,
            Self::Char => 1,
            Self::VarChar => 12,
            Self::LongVarChar => -1,
            Self::Date => 91,
            Self::Time => 92,
            Self::Timestamp => 93,
            Self::Binary => -2,
            Self::VarBinary => -3,
            Self::LongVarBinary => -4,
            Self::Null => 0,
            Self::Other => 1111,
            Self::JavaObject => 2000,
            Self::Distinct => 2001,
            Self::Struct => 2002,
            Self::Array => 2003,
            Self::Blob => 2004,
            Self::Clob => 2005,
            Self::Ref => 2006,
            Self::DataLink => 70,
            Self::Boolean => 16,
            Self::RowId => -8,
            Self::NChar => -15,
            Self::NVarChar => -9,
            Self::LongNVarChar => -16,
            Self::NClob => 2011,
            Self::SqlXml => 2009,
            Self::RefCursor => 2012,
            Self::TimeWithTimezone => 2013,
            Self::TimestampWithTimezone => 2014,
        }
    }
}
