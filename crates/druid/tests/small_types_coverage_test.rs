use chrono::{NaiveDate, NaiveDateTime};
use druid::core::{
    RdbcObject, RdbcParameter, RdbcParameterDate, RdbcParameterDecimal, RdbcParameterInt,
    RdbcParameterLong, RdbcParameterNull, RdbcParameterTimestamp, RdbcParameterValue, Value,
};
use druid::sql::{EofParserException, SqlType, WallFunctionStatValue};

// ── WallFunctionStatValue ──────────────────────────────────────

#[test]
fn wall_function_stat_value_to_map() {
    let v = WallFunctionStatValue {
        name: "count".to_owned(),
        invoke_count: 42,
    };
    let map = v.to_map();
    assert_eq!(map.get("name").unwrap().as_str().unwrap(), "count");
    assert_eq!(map.get("invokeCount").unwrap().as_u64().unwrap(), 42);
}

#[test]
fn wall_function_stat_value_clone_eq() {
    let v1 = WallFunctionStatValue {
        name: "sum".to_owned(),
        invoke_count: 10,
    };
    let v2 = v1.clone();
    assert_eq!(v1, v2);
}

// ── SqlType ────────────────────────────────────────────────────

#[test]
fn sql_type_java_name() {
    assert_eq!(SqlType::Select.java_name(), "SELECT");
    assert_eq!(SqlType::Insert.java_name(), "INSERT");
    assert_eq!(SqlType::Update.java_name(), "UPDATE");
    assert_eq!(SqlType::Delete.java_name(), "DELETE");
    assert_eq!(SqlType::CreateTable.java_name(), "CREATE_TABLE");
    assert_eq!(SqlType::DropTable.java_name(), "DROP_TABLE");
    assert_eq!(SqlType::AlterTable.java_name(), "ALTER_TABLE");
    assert_eq!(SqlType::ShowTables.java_name(), "SHOW_TABLES");
    assert_eq!(SqlType::Merge.java_name(), "MERGE");
    assert_eq!(SqlType::Truncate.java_name(), "TRUNCATE");
}

#[test]
fn sql_type_ordinal() {
    assert_eq!(SqlType::Select.ordinal(), 0);
    assert_eq!(SqlType::Update.ordinal(), 1);
    assert_eq!(SqlType::InsertSelect.ordinal(), 2);
}

#[test]
fn sql_type_value_of() {
    assert_eq!(SqlType::value_of("SELECT"), Some(SqlType::Select));
    assert_eq!(SqlType::value_of("INSERT"), Some(SqlType::Insert));
    assert_eq!(SqlType::value_of("NONEXISTENT"), None);
}

#[test]
fn sql_type_all_not_empty() {
    assert!(!SqlType::ALL.is_empty());
    assert!(SqlType::ALL.contains(&SqlType::Select));
    assert!(SqlType::ALL.contains(&SqlType::Error));
}

#[test]
fn sql_type_clone_copy_eq() {
    let t = SqlType::Select;
    let t2 = t;
    assert_eq!(t, t2);
}

// ── EofParserException ─────────────────────────────────────────

#[test]
fn eof_parser_exception_new() {
    let e = EofParserException::new();
    assert!(e.message().unwrap().contains("EOF"));
}

#[test]
fn eof_parser_exception_default() {
    let e = EofParserException::default();
    assert!(e.message().unwrap().contains("EOF"));
}

#[test]
fn eof_parser_exception_display() {
    let e = EofParserException::new();
    let s = format!("{}", e);
    assert!(s.contains("EOF"));
}

#[test]
fn eof_parser_exception_error_trait() {
    use std::error::Error;
    let e = EofParserException::new();
    let _: &dyn Error = &e;
}

// ── RdbcParameterNull ──────────────────────────────────────────

#[test]
fn rdbc_parameter_null_constants() {
    assert_eq!(RdbcParameterNull::CHAR.sql_type(), 1);
    assert_eq!(RdbcParameterNull::VARCHAR.sql_type(), 12);
    assert_eq!(RdbcParameterNull::NVARCHAR.sql_type(), -9);
    assert_eq!(RdbcParameterNull::BINARY.sql_type(), -2);
    assert_eq!(RdbcParameterNull::VARBINARY.sql_type(), -3);
    assert_eq!(RdbcParameterNull::TINYINT.sql_type(), 4);
    assert_eq!(RdbcParameterNull::SMALLINT.sql_type(), 5);
    assert_eq!(RdbcParameterNull::INTEGER.sql_type(), 4);
    assert_eq!(RdbcParameterNull::BIGINT.sql_type(), -5);
    assert_eq!(RdbcParameterNull::DECIMAL.sql_type(), 3);
    assert_eq!(RdbcParameterNull::NUMERIC.sql_type(), 2);
    assert_eq!(RdbcParameterNull::FLOAT.sql_type(), 6);
    assert_eq!(RdbcParameterNull::DOUBLE.sql_type(), 8);
    assert_eq!(RdbcParameterNull::NULL.sql_type(), 0);
    assert_eq!(RdbcParameterNull::DATE.sql_type(), 91);
    assert_eq!(RdbcParameterNull::TIME.sql_type(), 92);
    assert_eq!(RdbcParameterNull::TIMESTAMP.sql_type(), 93);
}

#[test]
fn rdbc_parameter_null_value_of_tinyint() {
    let p = RdbcParameterNull::value_of(-6);
    assert_eq!(p.sql_type(), 4);
}

#[test]
fn rdbc_parameter_null_value_of_normal() {
    let p = RdbcParameterNull::value_of(12);
    assert_eq!(p.sql_type(), 12);
}

#[test]
fn rdbc_parameter_null_value_is_none() {
    let p = RdbcParameterNull::VARCHAR;
    assert!(p.value().is_none());
    assert_eq!(p.length(), 0);
    assert!(p.calendar().is_none());
}

// ── RdbcParameterInt ───────────────────────────────────────────

#[test]
fn rdbc_parameter_int_value_of() {
    let p = RdbcParameterInt::value_of(42);
    match p.value() {
        Some(RdbcParameterValue::Object(RdbcObject::Integer(v))) => assert_eq!(v, 42),
        other => panic!("expected Integer, got {other:?}"),
    }
}

#[test]
fn rdbc_parameter_int_sql_type() {
    assert_eq!(RdbcParameterInt::value_of(0).sql_type(), 4);
}

#[test]
fn rdbc_parameter_int_length() {
    assert_eq!(RdbcParameterInt::value_of(0).length(), 0);
}

// ── RdbcParameterLong ──────────────────────────────────────────

#[test]
fn rdbc_parameter_long_value_of() {
    let p = RdbcParameterLong::value_of(999);
    match p.value() {
        Some(RdbcParameterValue::Object(RdbcObject::Long(v))) => assert_eq!(v, 999),
        other => panic!("expected Long, got {other:?}"),
    }
}

#[test]
fn rdbc_parameter_long_sql_type() {
    assert_eq!(RdbcParameterLong::value_of(0).sql_type(), -5);
}

// ── RdbcParameterDecimal ───────────────────────────────────────

#[test]
fn rdbc_parameter_decimal_value_of() {
    use bigdecimal::BigDecimal;
    let val: BigDecimal = "3.14".parse().unwrap();
    let p = RdbcParameterDecimal::value_of(Some(val.clone()));
    match p.value() {
        Some(RdbcParameterValue::Object(RdbcObject::BigDecimal(d))) => assert_eq!(d, val),
        other => panic!("expected BigDecimal, got {other:?}"),
    }
}

#[test]
fn rdbc_parameter_decimal_null() {
    let p = RdbcParameterDecimal::value_of(None);
    assert!(p.value().is_none());
}

#[test]
fn rdbc_parameter_decimal_sql_type() {
    let p = RdbcParameterDecimal::value_of(None);
    assert_eq!(p.sql_type(), 3);
}

// ── RdbcParameterDate ──────────────────────────────────────────

#[test]
fn rdbc_parameter_date_new_some() {
    let d = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let p = RdbcParameterDate::new(Some(d));
    match p.value() {
        Some(RdbcParameterValue::Object(RdbcObject::Date(v))) => assert_eq!(v, d),
        other => panic!("expected Date, got {other:?}"),
    }
}

#[test]
fn rdbc_parameter_date_new_none() {
    let p = RdbcParameterDate::new(None);
    assert!(p.value().is_none());
}

#[test]
fn rdbc_parameter_date_sql_type() {
    assert_eq!(RdbcParameterDate::new(None).sql_type(), 91);
}

// ── RdbcParameterTimestamp ─────────────────────────────────────

#[test]
fn rdbc_parameter_timestamp_new_some() {
    let dt = NaiveDateTime::parse_from_str("2024-01-15 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let p = RdbcParameterTimestamp::new(Some(dt));
    match p.value() {
        Some(RdbcParameterValue::Object(RdbcObject::Timestamp(v))) => assert_eq!(v, dt),
        other => panic!("expected Timestamp, got {other:?}"),
    }
}

#[test]
fn rdbc_parameter_timestamp_new_none() {
    let p = RdbcParameterTimestamp::new(None);
    assert!(p.value().is_none());
}

#[test]
fn rdbc_parameter_timestamp_sql_type() {
    assert_eq!(RdbcParameterTimestamp::new(None).sql_type(), 93);
}

// ── Struct (druid::sql::Struct) ───────────────────────────────

#[test]
fn struct_new_and_getters() {
    let s = druid::sql::Struct::new("ADDRESS", vec![Value::String("NY".to_owned())]);
    assert_eq!(s.sql_type_name(), "ADDRESS");
    assert_eq!(s.attributes().len(), 1);
}

#[test]
fn struct_clone_eq() {
    let s1 = druid::sql::Struct::new("T", vec![]);
    let s2 = s1.clone();
    assert_eq!(s1, s2);
}

#[test]
fn struct_debug() {
    let s = druid::sql::Struct::new("T", vec![]);
    let dbg = format!("{:?}", s);
    assert!(dbg.contains("Struct"));
}
