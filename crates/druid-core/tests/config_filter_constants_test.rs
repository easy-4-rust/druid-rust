extern crate druid_core as druid;
use druid::core::ConfigFilter;

#[test]
fn config_filter_constants() {
    assert_eq!(ConfigFilter::CONFIG_FILE, "config.file");
    assert_eq!(ConfigFilter::CONFIG_DECRYPT, "config.decrypt");
    assert_eq!(ConfigFilter::CONFIG_KEY, "config.decrypt.key");
    assert_eq!(ConfigFilter::SYS_PROP_CONFIG_FILE, "druid.config.file");
    assert_eq!(
        ConfigFilter::SYS_PROP_CONFIG_DECRYPT,
        "druid.config.decrypt"
    );
    assert_eq!(
        ConfigFilter::SYS_PROP_CONFIG_KEY,
        "druid.config.decrypt.key"
    );
    assert_eq!(ConfigFilter::CONNECTION_PROPERTIES, "connectionProperties");
    assert_eq!(ConfigFilter::PASSWORD, "password");
}

#[test]
fn config_filter_new() {
    let f = ConfigFilter::new();
    let _ = f;
}

#[test]
fn config_filter_with_runtime() {
    let client = reqwest::Client::new();
    let f = ConfigFilter::with_runtime(client, vec![]);
    let _ = f;
}
