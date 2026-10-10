use druid::sql::DriverManager;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::{ConnectionProperties, Driver, DriverAction};
use druid_rdbc::error::RdbcError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct RefusingDriver;

struct AcceptingButRefusingDriver;

#[async_trait::async_trait]
impl Driver for AcceptingButRefusingDriver {
    fn name(&self) -> &str {
        "accepting_but_refusing_driver"
    }

    fn accepts_url(&self, url: &str) -> bool {
        url.starts_with("rdbc:refusing-redaction-test://")
    }

    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        Ok(None)
    }
}

#[async_trait::async_trait]
impl Driver for RefusingDriver {
    fn name(&self) -> &str {
        "standard_driver_action_test"
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

struct CountingAction(Arc<AtomicUsize>);

impl DriverAction for CountingAction {
    fn deregister(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn canonical_action_preserves_manager_callback_identity_and_once_only_notification() {
    let driver: Arc<dyn Driver> = Arc::new(RefusingDriver);
    let original_count = Arc::new(AtomicUsize::new(0));
    let replacement_count = Arc::new(AtomicUsize::new(0));
    let canonical: Arc<dyn DriverAction> = Arc::new(CountingAction(Arc::clone(&original_count)));
    let historical: Arc<dyn druid::sql::driver_action::DriverAction> = Arc::clone(&canonical);
    let facade: Arc<dyn druid::sql::DriverAction> = historical;
    DriverManager::register_driver_with_action(Arc::clone(&driver), Some(facade));
    DriverManager::register_driver_with_action(
        Arc::clone(&driver),
        Some(Arc::new(CountingAction(Arc::clone(&replacement_count)))),
    );
    assert_eq!(original_count.load(Ordering::SeqCst), 0);
    assert!(DriverManager::deregister_driver(&driver));
    assert_eq!(original_count.load(Ordering::SeqCst), 1);
    assert_eq!(replacement_count.load(Ordering::SeqCst), 0);
    assert!(!DriverManager::deregister_driver(&driver));
    assert_eq!(original_count.load(Ordering::SeqCst), 1);
    assert_eq!(replacement_count.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn missing_driver_diagnostics_do_not_disclose_connection_url_secrets() {
    let missing_url = "rdbc:missing-redaction-test://localhost/db?password=canary-secret";
    let lookup_error = DriverManager::get_driver(missing_url)
        .err()
        .expect("missing driver must fail");
    assert!(!lookup_error.to_string().contains("canary-secret"));

    let refused_url = "rdbc:refusing-redaction-test://localhost/db?password=canary-secret";
    let driver: Arc<dyn Driver> = Arc::new(AcceptingButRefusingDriver);
    DriverManager::register_driver(Arc::clone(&driver));
    let connect_error = DriverManager::get_connection(refused_url)
        .await
        .err()
        .expect("refusing driver must return no connection");
    assert!(DriverManager::deregister_driver(&driver));
    assert!(!connect_error.to_string().contains("canary-secret"));
}
