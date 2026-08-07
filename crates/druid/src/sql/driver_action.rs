/// Cleanup callback invoked after a driver is deregistered from `DriverManager`.
///
/// Corresponds to Java: `java.sql.DriverAction`. Only the registering manager invokes it after
/// successful deregistration so the driver can release resources.
pub trait DriverAction: Send + Sync {
    /// Notifies the implementation that its driver has been deregistered.
    fn deregister(&self);
}
