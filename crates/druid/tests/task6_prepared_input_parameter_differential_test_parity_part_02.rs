//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
#![allow(clippy::approx_constant)]
//! Differential tests for `PreparedInputParameter` — Java `PreparedStatement.setXxx` semantics.
//!
//! Covers all variants of `PreparedInputParameter`, `scalar_value()`, `RdbcParameter` trait
//! (`value()`, `length()`, `calendar()`, `sql_type()`), and helper constructors.

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    PreparedInputParameter, PreparedTypeNameArgument, RdbcCalendar, RdbcCalendarArgument,
    RdbcCharacterLength, RdbcError, RdbcInputStream, RdbcObject, RdbcParameter, RdbcParameterType,
    RdbcParameterValue, RdbcReader, RdbcStreamLength, Value,
};
use std::str::FromStr;

// ── null / setNull ─────────────────────────────────────────────────

// ── boolean ────────────────────────────────────────────────────────

// ── byte ───────────────────────────────────────────────────────────

// ── short ──────────────────────────────────────────────────────────

// ── int ────────────────────────────────────────────────────────────

// ── long ───────────────────────────────────────────────────────────

// ── float ──────────────────────────────────────────────────────────

// ── double ─────────────────────────────────────────────────────────

// ── BigDecimal ─────────────────────────────────────────────────────

// ── String / NString ───────────────────────────────────────────────

// ── Bytes ──────────────────────────────────────────────────────────

// ── Date ───────────────────────────────────────────────────────────

// ── Time ───────────────────────────────────────────────────────────

// ── Timestamp ──────────────────────────────────────────────────────

// ── Object ─────────────────────────────────────────────────────────

// ── Stream variants ────────────────────────────────────────────────

// ── LOB variants ───────────────────────────────────────────────────

// ── RustValue ──────────────────────────────────────────────────────

// ── calendar() on non-date/time variants returns None ──────────────

// ── length() edge cases ────────────────────────────────────────────

// ── Clone and Debug ────────────────────────────────────────────────

// ── value() for all scalar variants ────────────────────────────────

#[test]
fn task6_prepared_input_parameter_differential_test_value_returns_object_for_float() {
    match PreparedInputParameter::Float(1.5).value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Float(v)) => assert_eq!(v, 1.5),
        _ => panic!("expected Float object"),
    }
}

#[test]
fn task6_prepared_input_parameter_differential_test_value_returns_object_for_double() {
    match PreparedInputParameter::Double(2.5).value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Double(v)) => assert_eq!(v, 2.5),
        _ => panic!("expected Double object"),
    }
}

#[test]
fn task6_prepared_input_parameter_differential_test_value_returns_object_for_decimal_some() {
    let bd = BigDecimal::from_str("99.9").unwrap();
    match PreparedInputParameter::BigDecimal(Some(bd.clone()))
        .value()
        .unwrap()
    {
        RdbcParameterValue::Object(RdbcObject::BigDecimal(v)) => assert_eq!(v, bd),
        _ => panic!("expected BigDecimal object"),
    }
}

#[test]
fn task6_prepared_input_parameter_differential_test_value_returns_none_for_decimal_none() {
    assert!(PreparedInputParameter::BigDecimal(None).value().is_none());
}

#[test]
fn task6_prepared_input_parameter_differential_test_value_returns_input_stream_for_ascii_stream() {
    let stream = RdbcInputStream::from_bytes(vec![65, 66]);
    let p = PreparedInputParameter::AsciiStream {
        stream: Some(stream),
        length: RdbcStreamLength::Int(2),
    };
    match p.value().unwrap() {
        RdbcParameterValue::InputStream(_) => {}
        _ => panic!("expected InputStream"),
    }
}

#[test]
fn task6_prepared_input_parameter_differential_test_value_returns_reader_for_character_stream() {
    let reader = RdbcReader::from_string("abc");
    let p = PreparedInputParameter::CharacterStream {
        reader: Some(reader),
        length: RdbcCharacterLength::Int(3),
    };
    match p.value().unwrap() {
        RdbcParameterValue::Reader(_) => {}
        _ => panic!("expected Reader"),
    }
}

#[test]
fn task6_prepared_input_parameter_differential_test_value_returns_none_for_empty_stream() {
    let p = PreparedInputParameter::BinaryStream {
        stream: None,
        length: RdbcStreamLength::Unspecified,
    };
    assert!(p.value().is_none());
}
