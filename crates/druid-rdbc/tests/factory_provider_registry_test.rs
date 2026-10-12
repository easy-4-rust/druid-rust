use druid_rdbc::connection::Connection;
use druid_rdbc::driver::{ConnectionFactory, ConnectionProperties, DriverRegistry};
use druid_rdbc::RdbcError;
use std::sync::Arc;

struct NamedFactory(String);

#[async_trait::async_trait]
impl ConnectionFactory for NamedFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        unreachable!("registry lookup must not open a connection")
    }

    fn name(&self) -> &str {
        &self.0
    }
}

#[test]
fn named_provider_lookup_preserves_configuration_and_replaces_registration() {
    let mut registry = DriverRegistry::new();
    assert!(registry.factory_provider("sqlx").is_none());

    registry.register_factory_provider(
        "sqlx",
        Arc::new(|_: &str, _: &ConnectionProperties, _: Option<&str>| {
            panic!("replaced provider must never execute")
        }),
    );
    registry.register_factory_provider(
        "sqlx",
        Arc::new(
            |url: &str, properties: &ConnectionProperties, query: Option<&str>| {
                assert_eq!(url, "sqlite::memory:");
                assert_eq!(properties.get("mode").map(String::as_str), Some("memory"));
                assert_eq!(query, Some("SELECT 1"));
                Arc::new(NamedFactory("new".to_owned())) as Arc<dyn ConnectionFactory>
            },
        ),
    );

    let properties = ConnectionProperties::from([("mode".to_owned(), "memory".to_owned())]);
    let factory = registry
        .factory_provider("sqlx")
        .expect("registered provider")
        .factory("sqlite::memory:", &properties, Some("SELECT 1"));
    assert_eq!(factory.name(), "new");
    assert_eq!(registry.factory_provider_count(), 1);
}
