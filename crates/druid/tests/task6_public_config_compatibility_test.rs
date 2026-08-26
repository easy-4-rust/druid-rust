#![allow(deprecated)]

use druid::core::DriverProperty;
use druid::dynamic::{DataSourceCreator, HighAvailableDataSource};
use druid::pool::{DruidPool, PoolInnerConfig};
use std::time::Duration;

#[test]
fn deprecated_timeout_entrypoints_delegate_to_driver_named_configuration() {
    let config = PoolInnerConfig::default();
    assert_eq!(config.physical_connection_timeout, None);
    assert_eq!(config.driver_connection_timeout, None);

    let _builder = DruidPool::builder()
        .physical_connection_timeout(Duration::from_secs(30))
        .phy_timeout(Duration::from_secs(60));

    let data_source = HighAvailableDataSource::new("compat", DataSourceCreator::noop_for_test());
    data_source.set_physical_timeout(Some(Duration::from_secs(15)));
    data_source.set_physical_timeout(None);
}

#[test]
fn druid_driver_property_alias_retains_the_semantic_constructor() {
    let property = DriverProperty::new("ssl", None);
    assert_eq!(property.name, "ssl");
    assert_eq!(property.description, None);
    assert!(!property.required);
    assert!(property.choices.is_empty());
}
