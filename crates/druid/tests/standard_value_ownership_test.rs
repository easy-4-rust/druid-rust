// 标准值对象唯一归属及固定 Java 8 数值合同。
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::PseudoColumnUsage;
use druid_rdbc::types::{JdbcType, RowId, SqlType};
use std::any::TypeId;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[test]
fn canonical_jdbc_type_preserves_every_java8_name_vendor_and_number() {
    let cases = [
        (JdbcType::Bit, "BIT", -7),
        (JdbcType::TinyInt, "TINYINT", -6),
        (JdbcType::SmallInt, "SMALLINT", 5),
        (JdbcType::Integer, "INTEGER", 4),
        (JdbcType::BigInt, "BIGINT", -5),
        (JdbcType::Float, "FLOAT", 6),
        (JdbcType::Real, "REAL", 7),
        (JdbcType::Double, "DOUBLE", 8),
        (JdbcType::Numeric, "NUMERIC", 2),
        (JdbcType::Decimal, "DECIMAL", 3),
        (JdbcType::Char, "CHAR", 1),
        (JdbcType::VarChar, "VARCHAR", 12),
        (JdbcType::LongVarChar, "LONGVARCHAR", -1),
        (JdbcType::Date, "DATE", 91),
        (JdbcType::Time, "TIME", 92),
        (JdbcType::Timestamp, "TIMESTAMP", 93),
        (JdbcType::Binary, "BINARY", -2),
        (JdbcType::VarBinary, "VARBINARY", -3),
        (JdbcType::LongVarBinary, "LONGVARBINARY", -4),
        (JdbcType::Null, "NULL", 0),
        (JdbcType::Other, "OTHER", 1111),
        (JdbcType::JavaObject, "JAVA_OBJECT", 2000),
        (JdbcType::Distinct, "DISTINCT", 2001),
        (JdbcType::Struct, "STRUCT", 2002),
        (JdbcType::Array, "ARRAY", 2003),
        (JdbcType::Blob, "BLOB", 2004),
        (JdbcType::Clob, "CLOB", 2005),
        (JdbcType::Ref, "REF", 2006),
        (JdbcType::DataLink, "DATALINK", 70),
        (JdbcType::Boolean, "BOOLEAN", 16),
        (JdbcType::RowId, "ROWID", -8),
        (JdbcType::NChar, "NCHAR", -15),
        (JdbcType::NVarChar, "NVARCHAR", -9),
        (JdbcType::LongNVarChar, "LONGNVARCHAR", -16),
        (JdbcType::NClob, "NCLOB", 2011),
        (JdbcType::SqlXml, "SQLXML", 2009),
        (JdbcType::RefCursor, "REF_CURSOR", 2012),
        (JdbcType::TimeWithTimezone, "TIME_WITH_TIMEZONE", 2013),
        (
            JdbcType::TimestampWithTimezone,
            "TIMESTAMP_WITH_TIMEZONE",
            2014,
        ),
    ];
    assert_eq!(cases.len(), 39);
    for (canonical, name, number) in cases {
        let legacy: druid_rdbc::types::RdbcType = canonical;
        let product: druid::sql::RdbcType = legacy;
        assert_eq!(canonical.name(), name);
        assert_eq!(canonical.vendor(), "java.sql");
        assert_eq!(canonical.vendor_type_number(), number);
        assert_eq!(JdbcType::value_of(number).unwrap(), canonical);
        assert_eq!(product, canonical);
    }
    assert_eq!(
        TypeId::of::<JdbcType>(),
        TypeId::of::<druid_rdbc::types::RdbcType>()
    );
    for unknown in [i32::MIN, i32::MAX, 9999, -9999] {
        assert!(matches!(
            JdbcType::value_of(unknown),
            Err(RdbcError::InvalidArgument(_))
        ));
    }
}

fn row_id_hash(value: &RowId) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn canonical_row_id_preserves_byte_identity_hash_and_copy_isolation() {
    let canonical = RowId::new(vec![0, 128, 255]);
    let mut copied = canonical.get_bytes();
    copied[0] = 7;
    assert_eq!(canonical.bytes(), &[0, 128, 255]);
    assert_ne!(copied, canonical.get_bytes());
    let legacy: druid_rdbc::types::RdbcRowId = canonical.clone();
    let product: druid::sql::RowId = legacy;
    assert_eq!(product, canonical);
    assert_eq!(row_id_hash(&product), row_id_hash(&canonical));
    assert_ne!(canonical, RowId::new(vec![0, 128, 254]));
    assert_eq!(RowId::new(Vec::new()).get_bytes(), Vec::<u8>::new());
    assert_eq!(
        TypeId::of::<RowId>(),
        TypeId::of::<druid_rdbc::types::RdbcRowId>()
    );
}

#[test]
fn canonical_row_id_preserves_existing_resource_trace_debug_representation() {
    let canonical = RowId::new(vec![0, 128, 255]);
    let legacy: druid_rdbc::types::RdbcRowId = canonical.clone();
    let product: druid::sql::RowId = canonical.clone();
    let expected = "Some(RdbcRowId { bytes: [0, 128, 255] })";
    assert_eq!(format!("{:?}", Some(canonical)), expected);
    assert_eq!(format!("{:?}", Some(legacy)), expected);
    assert_eq!(format!("{:?}", Some(product)), expected);
}

#[test]
fn canonical_pseudo_column_usage_preserves_all_four_distinct_states() {
    let states = [
        PseudoColumnUsage::SelectListOnly,
        PseudoColumnUsage::WhereClauseOnly,
        PseudoColumnUsage::NoUsageRestrictions,
        PseudoColumnUsage::UsageUnknown,
    ];
    for (index, canonical) in states.into_iter().enumerate() {
        let product: druid::sql::PseudoColumnUsage = canonical;
        assert_eq!(product, canonical);
        for (other_index, other) in states.into_iter().enumerate() {
            assert_eq!(canonical == other, index == other_index);
        }
    }
    assert_eq!(
        TypeId::of::<PseudoColumnUsage>(),
        TypeId::of::<druid::sql::PseudoColumnUsage>()
    );
}
