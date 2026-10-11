use druid::sql::DriverManager;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::{
    ConnectionProperties, Driver, DriverAction, DriverExtensionDescriptor, DriverRegistry,
};
use druid_rdbc::error::RdbcError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

struct RefusingDriver;

struct AcceptingButRefusingDriver;

struct SecondCandidateDriver;

struct ReentrantDriver {
    registered: Arc<dyn Driver>,
    callback_completed: Arc<AtomicUsize>,
    registration: Mutex<Option<std::thread::JoinHandle<()>>>,
}

#[async_trait::async_trait]
impl Driver for ReentrantDriver {
    fn name(&self) -> &str {
        "reentrant_driver"
    }

    fn accepts_url(&self, url: &str) -> bool {
        if !url.starts_with("rdbc:reentrant-driver-test://") {
            return false;
        }
        let (sender, receiver) = mpsc::channel();
        let registered = Arc::clone(&self.registered);
        let handle = std::thread::spawn(move || {
            DriverManager::register_driver(registered);
            let _ = sender.send(());
        });
        let completed = receiver.recv_timeout(Duration::from_secs(2)).is_ok();
        self.callback_completed
            .store(usize::from(completed), Ordering::SeqCst);
        *self.registration.lock().unwrap() = Some(handle);
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

#[async_trait::async_trait]
impl Driver for AcceptingButRefusingDriver {
    fn name(&self) -> &str {
        "accepting_but_refusing_driver"
    }

    fn accepts_url(&self, url: &str) -> bool {
        url.starts_with("rdbc:refusing-redaction-test://")
            || url.starts_with("rdbc:next-candidate-test://")
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
impl Driver for SecondCandidateDriver {
    fn name(&self) -> &str {
        "second_candidate_driver"
    }

    fn accepts_url(&self, url: &str) -> bool {
        url.starts_with("rdbc:next-candidate-test://")
    }

    async fn connect(
        &self,
        _url: &str,
        _properties: &ConnectionProperties,
    ) -> Result<Option<Box<dyn Connection>>, RdbcError> {
        Err(RdbcError::DriverError(
            "second candidate reached".to_owned(),
        ))
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

#[tokio::test]
async fn connection_lookup_continues_after_first_driver_returns_none() {
    let first: Arc<dyn Driver> = Arc::new(AcceptingButRefusingDriver);
    let second: Arc<dyn Driver> = Arc::new(SecondCandidateDriver);
    let url = "rdbc:next-candidate-test://localhost/db";
    DriverManager::register_driver(Arc::clone(&first));
    DriverManager::register_driver(Arc::clone(&second));

    let result = DriverManager::get_connection(url).await;
    let properties_result =
        DriverManager::get_connection_with_properties(url, &Default::default()).await;
    let credentials_result =
        DriverManager::get_connection_with_credentials(url, "user", "password").await;
    let mut standard = DriverRegistry::new();
    standard.register(DriverExtensionDescriptor::new(
        "first",
        Arc::clone(&first),
        None,
    ));
    standard.register(DriverExtensionDescriptor::new(
        "second",
        Arc::clone(&second),
        None,
    ));
    let standard_result = standard.connect(url, &ConnectionProperties::new()).await;
    assert!(DriverManager::deregister_driver(&first));
    assert!(DriverManager::deregister_driver(&second));
    for result in [result, properties_result, credentials_result] {
        assert!(matches!(
            result,
            Err(RdbcError::DriverError(ref message)) if message == "second candidate reached"
        ));
    }
    assert!(matches!(
        standard_result,
        Err(RdbcError::DriverError(ref message)) if message == "second candidate reached"
    ));
}

#[test]
fn standard_registry_selects_first_accepted_driver_without_connecting() {
    let first: Arc<dyn Driver> = Arc::new(RefusingDriver);
    let second: Arc<dyn Driver> = Arc::new(AcceptingButRefusingDriver);
    let third: Arc<dyn Driver> = Arc::new(SecondCandidateDriver);
    let mut registry = DriverRegistry::new();
    for driver in [&first, &second, &third] {
        registry.register(DriverExtensionDescriptor::new(
            driver.name(),
            Arc::clone(driver),
            None,
        ));
    }

    let selected = registry
        .first_accepted_driver("rdbc:next-candidate-test://localhost/db")
        .expect("second descriptor accepts the URL");
    assert!(Arc::ptr_eq(&selected, &second));
    assert!(registry
        .first_accepted_driver("rdbc:missing-driver-test://localhost/db")
        .is_none());
}

#[test]
fn driver_callback_can_register_without_registry_lock_reentrancy() {
    let registered: Arc<dyn Driver> = Arc::new(RefusingDriver);
    let completed = Arc::new(AtomicUsize::new(0));
    let probe = Arc::new(ReentrantDriver {
        registered: Arc::clone(&registered),
        callback_completed: Arc::clone(&completed),
        registration: Mutex::new(None),
    });
    let driver: Arc<dyn Driver> = probe.clone();
    DriverManager::register_driver(Arc::clone(&driver));
    let result = DriverManager::get_driver("rdbc:reentrant-driver-test://localhost/db");
    probe
        .registration
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .join()
        .unwrap();
    assert!(DriverManager::deregister_driver(&driver));
    assert!(DriverManager::deregister_driver(&registered));
    assert!(result.is_err());
    assert_eq!(completed.load(Ordering::SeqCst), 1);
}
