use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use parking_lot::RwLock;

use super::{Driver, DriverAction, RdbcLogWriter};
use crate::core::{DruidError, PhysicalConnection};

#[derive(Clone)]
struct RegisteredDriver {
    driver: Arc<dyn Driver>,
    action: Option<Arc<dyn DriverAction>>,
}

/// Basic service that manages process-wide RDBC drivers and opens connections.
///
/// Corresponds to Java: `java.sql.DriverManager`. Drivers are considered in registration order;
/// the first driver accepting a URL opens the connection. A global login timeout bounds that
/// operation, and the configured writer receives diagnostics. Server code usually uses
/// `DataSource`, but both entry points retain the same driver-error semantics.
pub struct DriverManager;

impl DriverManager {
    fn registry() -> &'static RwLock<Vec<RegisteredDriver>> {
        static REGISTRY: OnceLock<RwLock<Vec<RegisteredDriver>>> = OnceLock::new();
        REGISTRY.get_or_init(|| RwLock::new(Vec::new()))
    }

    fn login_timeout_seconds() -> &'static AtomicU64 {
        static LOGIN_TIMEOUT_SECONDS: AtomicU64 = AtomicU64::new(0);
        &LOGIN_TIMEOUT_SECONDS
    }

    fn log_writer_slot() -> &'static RwLock<Option<RdbcLogWriter>> {
        static LOG_WRITER: OnceLock<RwLock<Option<RdbcLogWriter>>> = OnceLock::new();
        LOG_WRITER.get_or_init(|| RwLock::new(None))
    }

    /// Registers a driver instance.
    ///
    /// The same `Arc` instance is not inserted twice. Corresponds to Java:
    /// `DriverManager#registerDriver(Driver)`.
    pub fn register_driver(driver: Arc<dyn Driver>) {
        Self::register_driver_with_action(driver, None);
    }

    /// Registers a driver and its deregistration callback.
    ///
    /// `action` runs once after successful removal. Duplicate registration does not replace an
    /// existing callback. Corresponds to Java: `registerDriver(Driver, DriverAction)`.
    pub fn register_driver_with_action(
        driver: Arc<dyn Driver>,
        action: Option<Arc<dyn DriverAction>>,
    ) {
        let mut registry = Self::registry().write();
        if !registry
            .iter()
            .any(|entry| Arc::ptr_eq(&entry.driver, &driver))
        {
            registry.push(RegisteredDriver { driver, action });
        }
    }

    /// Deregisters the specified driver instance and invokes its callback.
    ///
    /// Returns whether the exact `Arc` instance was removed. Corresponds to Java:
    /// `DriverManager#deregisterDriver`.
    pub fn deregister_driver(driver: &Arc<dyn Driver>) -> bool {
        let removed = {
            let mut registry = Self::registry().write();
            registry
                .iter()
                .position(|entry| Arc::ptr_eq(&entry.driver, driver))
                .map(|index| registry.remove(index))
        };
        if let Some(entry) = removed {
            if let Some(action) = entry.action {
                action.deregister();
            }
            true
        } else {
            false
        }
    }

    /// Returns a snapshot of visible drivers in registration order.
    ///
    /// Later registration changes do not mutate the returned `Vec`. Corresponds to Java:
    /// `getDrivers`.
    #[must_use]
    pub fn drivers() -> Vec<Arc<dyn Driver>> {
        Self::registry()
            .read()
            .iter()
            .map(|entry| Arc::clone(&entry.driver))
            .collect()
    }

    /// Returns visible drivers in registration order; `snake_case` form of Java `getDrivers`.
    #[must_use]
    pub fn get_drivers() -> Vec<Arc<dyn Driver>> {
        Self::drivers()
    }

    /// Returns the first driver that declares it understands `url`.
    ///
    /// Returns `DriverError` when no suitable driver is registered.
    pub fn get_driver(url: &str) -> Result<Arc<dyn Driver>, DruidError> {
        Self::driver_for(url)
    }

    /// Sets the login timeout, in seconds, for all manager connection attempts.
    ///
    /// Zero means no limit. This value does not control statement execution time. Corresponds
    /// to Java: `DriverManager#setLoginTimeout`.
    pub fn set_login_timeout(seconds: u64) {
        Self::login_timeout_seconds().store(seconds, Ordering::Release);
    }

    /// Returns the global driver login timeout in seconds; zero means no limit.
    #[must_use]
    pub fn login_timeout() -> u64 {
        Self::login_timeout_seconds().load(Ordering::Acquire)
    }

    /// Returns the global login timeout; `snake_case` form of Java `getLoginTimeout`.
    #[must_use]
    pub fn get_login_timeout() -> u64 {
        Self::login_timeout()
    }

    /// Sets the process-wide RDBC log writer; `None` disables output.
    ///
    /// Diagnostics may contain database structure or data, so callers must enforce access and
    /// redaction. Corresponds to Java: `DriverManager#setLogWriter`.
    pub fn set_log_writer(writer: Option<RdbcLogWriter>) {
        *Self::log_writer_slot().write() = writer;
    }

    /// Returns the process-wide RDBC log writer, or `None` when disabled.
    #[must_use]
    pub fn log_writer() -> Option<RdbcLogWriter> {
        Self::log_writer_slot().read().clone()
    }

    /// Returns the process-wide writer; `snake_case` form of Java `getLogWriter`.
    #[must_use]
    pub fn get_log_writer() -> Option<RdbcLogWriter> {
        Self::log_writer()
    }

    /// Writes one line to the RDBC diagnostic writer.
    ///
    /// This is a no-op when no writer is configured. I/O failure returns `DruidError::Other`.
    /// Corresponds to Java: `DriverManager#println`.
    pub fn println(message: &str) -> Result<(), DruidError> {
        if let Some(writer) = Self::log_writer() {
            use std::io::Write as _;
            writeln!(writer.lock(), "{message}")
                .map_err(|error| DruidError::Other(format!("RDBC log writer failed: {error}")))?;
        }
        Ok(())
    }

    /// Opens a physical connection with the first driver that accepts `url`.
    ///
    /// Returns an unpooled connection. Missing drivers, driver failures, and login timeouts retain
    /// their error semantics. Corresponds to Java: `getConnection(String)`.
    pub async fn get_connection(url: &str) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let driver = Self::driver_for(url)?;
        Self::with_login_timeout(driver.connect(url)).await
    }

    /// Opens a physical connection using RDBC property semantics.
    ///
    /// `info` is passed to the driver. The driver resolves properties duplicated in the URL.
    /// Corresponds to Java: `getConnection(String, Properties)`.
    pub async fn get_connection_with_properties(
        url: &str,
        info: &HashMap<String, String>,
    ) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let driver = Self::driver_for(url)?;
        Self::with_login_timeout(driver.connect_with_properties(url, info)).await
    }

    /// Opens a physical connection with a URL, user name, and password.
    ///
    /// Credentials are passed only to the driver and must not be logged. Corresponds to Java:
    /// `getConnection(String, String, String)`.
    pub async fn get_connection_with_credentials(
        url: &str,
        username: &str,
        password: &str,
    ) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        let driver = Self::driver_for(url)?;
        Self::with_login_timeout(driver.connect_with_auth(url, username, password)).await
    }

    fn driver_for(url: &str) -> Result<Arc<dyn Driver>, DruidError> {
        Self::registry()
            .read()
            .iter()
            .find(|entry| entry.driver.accepts_url(url))
            .map(|entry| Arc::clone(&entry.driver))
            .ok_or_else(|| DruidError::DriverError(format!("No suitable driver found for {url}")))
    }

    async fn with_login_timeout<F>(future: F) -> Result<Box<dyn PhysicalConnection>, DruidError>
    where
        F: std::future::Future<Output = Result<Box<dyn PhysicalConnection>, DruidError>>,
    {
        let seconds = Self::login_timeout();
        if seconds == 0 {
            future.await
        } else {
            tokio::time::timeout(Duration::from_secs(seconds), future)
                .await
                .map_err(|_| DruidError::LoginTimeout)?
        }
    }
}
