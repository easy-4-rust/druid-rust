use druid_rdbc::types::{JdbcType, SqlType};
fn main() {
    let types = [
        JdbcType::Bit,
        JdbcType::TinyInt,
        JdbcType::SmallInt,
        JdbcType::Integer,
        JdbcType::BigInt,
        JdbcType::Float,
        JdbcType::Real,
        JdbcType::Double,
        JdbcType::Numeric,
        JdbcType::Decimal,
        JdbcType::Char,
        JdbcType::VarChar,
        JdbcType::LongVarChar,
        JdbcType::Date,
        JdbcType::Time,
        JdbcType::Timestamp,
        JdbcType::Binary,
        JdbcType::VarBinary,
        JdbcType::LongVarBinary,
        JdbcType::Null,
        JdbcType::Other,
        JdbcType::JavaObject,
        JdbcType::Distinct,
        JdbcType::Struct,
        JdbcType::Array,
        JdbcType::Blob,
        JdbcType::Clob,
        JdbcType::Ref,
        JdbcType::DataLink,
        JdbcType::Boolean,
        JdbcType::RowId,
        JdbcType::NChar,
        JdbcType::NVarChar,
        JdbcType::LongNVarChar,
        JdbcType::NClob,
        JdbcType::SqlXml,
        JdbcType::RefCursor,
        JdbcType::TimeWithTimezone,
        JdbcType::TimestampWithTimezone,
    ];
    for value in types {
        assert_eq!(JdbcType::value_of(value.vendor_type_number()).unwrap(), value);
        println!("TYPE\t{}\t{}\t{}", value.name(), value.vendor(), value.vendor_type_number());
    }
    for number in [i32::MIN, i32::MAX, 9999, -9999] {
        assert!(matches!(JdbcType::value_of(number), Err(druid_rdbc::error::RdbcError::InvalidArgument(_))));
        println!("UNKNOWN\t{}\tInvalidArgument", number);
    }
}
