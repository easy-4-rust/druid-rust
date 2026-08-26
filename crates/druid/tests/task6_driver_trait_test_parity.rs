//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
use druid::core::{Connection, Driver, DriverProperty, RdbcError};
use std::collections::BTreeMap;

struct MockDriver;

#[async_trait::async_trait]
impl Driver for MockDriver {
    fn name(&self) -> &'static str {
        "mock"
    }
    fn accepts_url(&self, _url: &str) -> bool {
        true
    }
    async fn connect(
        &self,
        _url: &str,
        _properties: &druid_rdbc::driver::ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        Err(RdbcError::Other("not implemented".to_owned()))
    }
}

// ── Driver trait default methods ───────────────────────────────

// ── DriverProperty ─────────────────────────────────────────────

#[test]
fn task6_driver_trait_test_driver_accepts_url_default() {
    let d = MockDriver;
    assert!(d.accepts_url("any://url"));
}

#[test]
fn task6_driver_trait_test_driver_property_info_default() {
    let d = MockDriver;
    let info = d.property_info("any://url", &BTreeMap::new()).unwrap();
    assert!(info.is_empty());
}

#[test]
fn task6_driver_trait_test_driver_get_property_info_delegates() {
    let d = MockDriver;
    let info = d.property_info("any://url", &BTreeMap::new()).unwrap();
    assert!(info.is_empty());
}

#[test]
fn task6_driver_trait_test_driver_major_version_default() {
    let d = MockDriver;
    assert_eq!(d.major_version(), 0);
    assert_eq!(d.major_version(), 0);
}

#[test]
fn task6_driver_trait_test_driver_minor_version_default() {
    let d = MockDriver;
    assert_eq!(d.minor_version(), 0);
    assert_eq!(d.minor_version(), 0);
}

#[test]
fn task6_driver_trait_test_driver_rdbc_compliant_default() {
    let d = MockDriver;
    assert!(!d.rdbc_compliant());
}

#[test]
fn task6_driver_trait_test_driver_parent_logger_default() {
    let d = MockDriver;
    assert_eq!(d.parent_logger().unwrap(), "druid::rdbc");
    assert_eq!(d.get_parent_logger().unwrap(), "druid::rdbc");
}

#[test]
fn task6_driver_trait_test_driver_name() {
    let d = MockDriver;
    assert_eq!(d.name(), "mock");
}

#[test]
fn task6_driver_trait_test_driver_property_new_basic() {
    let p = DriverProperty::new("host", Some("localhost".to_owned()));
    assert_eq!(p.name, "host");
    assert_eq!(p.value, Some("localhost".to_owned()));
    assert!(p.description.is_none());
    assert!(!p.required);
    assert!(p.choices.is_empty());
}

#[test]
fn task6_driver_trait_test_driver_property_new_none() {
    let p = DriverProperty::new("password", None);
    assert_eq!(p.name, "password");
    assert!(p.value.is_none());
}

#[test]
fn task6_driver_trait_test_driver_property_custom_fields() {
    let mut p = DriverProperty::new("ssl", None);
    p.description = Some("Enable SSL".to_owned());
    p.required = true;
    p.choices = vec!["true".to_owned(), "false".to_owned()];
    assert_eq!(p.description, Some("Enable SSL".to_owned()));
    assert!(p.required);
    assert_eq!(p.choices.len(), 2);
}

#[test]
fn task6_driver_trait_test_driver_property_clone_eq() {
    let p1 = DriverProperty::new("host", Some("localhost".to_owned()));
    let p2 = p1.clone();
    assert_eq!(p1, p2);
}

#[test]
fn task6_driver_trait_test_driver_property_debug() {
    let p = DriverProperty::new("port", Some("5432".to_owned()));
    let dbg = format!("{p:?}");
    assert!(dbg.contains("DriverProperty"));
}
