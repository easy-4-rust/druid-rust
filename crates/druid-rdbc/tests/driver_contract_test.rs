use druid_rdbc::connection::Connection;
use druid_rdbc::driver::{ConnectionProperties, Driver, DriverPropertyInfo};
use druid_rdbc::RdbcError;

struct NamedDriver;

#[async_trait::async_trait]
impl Driver for NamedDriver {
    fn name(&self) -> &str {
        "named-driver"
    }

    fn accepts_url(&self, _url: &str) -> bool {
        false
    }

    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        Ok(None)
    }
}

#[test]
fn driver_exposes_its_registry_name() {
    assert_eq!(NamedDriver.name(), "named-driver");
}

#[test]
fn driver_defaults_preserve_jdbc_compliance_and_parent_logger() {
    assert!(!NamedDriver.rdbc_compliant());
    assert_eq!(NamedDriver.parent_logger().unwrap(), "druid::rdbc");
    assert_eq!(NamedDriver.get_parent_logger().unwrap(), "druid::rdbc");
}

#[test]
fn driver_property_constructor_preserves_jdbc_defaults() {
    let mut property = DriverPropertyInfo::new("ssl", None);

    assert_eq!(property.name, "ssl");
    assert_eq!(property.value, None);
    assert_eq!(property.description, None);
    assert!(!property.required);
    assert!(property.choices.is_empty());

    property.description = Some("Enable TLS".to_owned());
    property.required = true;
    property.choices = vec!["true".to_owned(), "false".to_owned()];

    assert_eq!(property.description.as_deref(), Some("Enable TLS"));
    assert_eq!(property.choices, ["true", "false"]);
}
