//! `ParameterMetaData` + `ResultSetMetaDataProxyImpl` 差分测试
//! （C9 批次：sql + core 0% 文件）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。

use druid::core::{ProxyAttributeValue, ResultSetMetaData, ResultSetMetaDataProxyImpl};
use druid::sql::{ParameterMetaData, ParameterMode, ParameterNullability, RdbcType};

// ── ParameterMetaData（Java ParameterMetaData）─────────────────

/// new + `parameter_count` / `get_parameter_count`。
#[test]
fn parameter_meta_data_new_and_count() {
    let types = vec![RdbcType::Integer, RdbcType::VarChar, RdbcType::Boolean];
    let meta = ParameterMetaData::new(types);
    assert_eq!(meta.parameter_count(), 3);
    assert_eq!(meta.get_parameter_count(), 3);
}

/// `parameter_type` / `get_parameter_type` (1-based JDBC indexing)。
#[test]
fn parameter_meta_data_type_access() {
    let types = vec![RdbcType::Integer, RdbcType::VarChar, RdbcType::Boolean];
    let meta = ParameterMetaData::new(types);
    assert_eq!(meta.parameter_type(1), Some(RdbcType::Integer));
    assert_eq!(meta.parameter_type(2), Some(RdbcType::VarChar));
    assert_eq!(meta.parameter_type(3), Some(RdbcType::Boolean));
    assert_eq!(meta.parameter_type(0), None, "0 is out of range (1-based)");
    assert_eq!(meta.parameter_type(4), None, "out of range");
    let vendor = meta.get_parameter_type(1).unwrap();
    assert!(
        vendor > 0,
        "Integer vendor type should be positive: {vendor}"
    );
    assert!(meta.get_parameter_type(0).is_none());
}

/// `parameter_type_name` / `get_parameter_type_name` (1-based)。
#[test]
fn parameter_meta_data_type_name() {
    let types = vec![RdbcType::Integer, RdbcType::VarChar];
    let meta = ParameterMetaData::new(types);
    let name = meta.parameter_type_name(1);
    assert!(name.is_some(), "Integer should have a name");
    let name2 = meta.get_parameter_type_name(1);
    assert_eq!(name, name2);
    assert!(meta.parameter_type_name(0).is_none());
    assert!(meta.parameter_type_name(99).is_none());
}

/// `parameter_class_name` / `get_parameter_class_name` (1-based)。
#[test]
fn parameter_meta_data_class_name() {
    let types = vec![RdbcType::Integer, RdbcType::VarChar];
    let meta = ParameterMetaData::new(types);
    let class = meta.parameter_class_name(1);
    assert!(class.is_some(), "Integer should have a class name");
    let class2 = meta.get_parameter_class_name(1);
    assert_eq!(class, class2);
    assert!(meta.parameter_class_name(0).is_none());
}

/// `parameter_mode` / `get_parameter_mode` (1-based)。
#[test]
fn parameter_meta_data_mode() {
    let types = vec![RdbcType::Integer];
    let meta = ParameterMetaData::new(types);
    let mode = meta.parameter_mode(1);
    assert!(mode.is_some(), "should have a mode");
    let mode2 = meta.get_parameter_mode(1);
    assert_eq!(mode, mode2);
    assert!(meta.parameter_mode(0).is_none());
}

/// nullable (1-based)。
#[test]
fn parameter_meta_data_nullable() {
    let types = vec![RdbcType::Integer];
    let meta = ParameterMetaData::new(types);
    let nullable = meta.nullable(1);
    assert!(nullable.is_some(), "should have nullability");
    assert!(meta.nullable(0).is_none());
    assert!(meta.nullable(99).is_none());
}

/// `ParameterMode` 枚举变体。
#[test]
fn parameter_mode_variants() {
    assert_eq!(ParameterMode::In as i32, 1);
    assert_eq!(ParameterMode::InOut as i32, 2);
    assert_eq!(ParameterMode::Out as i32, 4);
    assert_eq!(ParameterMode::Unknown as i32, 0);
}

/// `ParameterNullability` 枚举变体。
#[test]
fn parameter_nullability_variants() {
    assert_eq!(ParameterNullability::NoNulls as i32, 0);
    assert_eq!(ParameterNullability::Nullable as i32, 1);
    assert_eq!(ParameterNullability::Unknown as i32, 2);
}

/// 空参数列表。
#[test]
fn parameter_meta_data_empty() {
    let meta = ParameterMetaData::new(vec![]);
    assert_eq!(meta.parameter_count(), 0);
    assert!(meta.parameter_type(0).is_none());
}

// ── ResultSetMetaDataProxyImpl（Java ResultSetMetaDataProxy）───

/// new + `attributes_size` + attributes + attribute。
#[test]
fn result_set_meta_data_proxy_attributes() {
    let base = ResultSetMetaData::new(vec![]);
    let proxy = ResultSetMetaDataProxyImpl::new(base, 1, 1);
    assert_eq!(proxy.attributes_size(), 0);
    assert!(proxy.attributes().is_empty());
    assert!(proxy.attribute("key").is_none());
}

/// `put_attribute` + `clear_attributes`。
#[test]
fn result_set_meta_data_proxy_put_and_clear() {
    let base = ResultSetMetaData::new(vec![]);
    let proxy = ResultSetMetaDataProxyImpl::new(base, 1, 1);
    proxy.put_attribute(
        "key".to_owned(),
        ProxyAttributeValue::new("value".to_owned()),
    );
    assert_eq!(proxy.attributes_size(), 1);
    assert!(proxy.attribute("key").is_some());

    proxy.clear_attributes();
    assert_eq!(proxy.attributes_size(), 0);
    assert!(proxy.attribute("key").is_none());
}
