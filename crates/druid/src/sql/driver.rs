/// Connection protocol implemented by every RDBC database driver.
///
/// Corresponds to Java: `java.sql.Driver`. `accepts_url` tests URL support, `connect` opens a
/// physical connection, property information supports configuration tools, and version and
/// compliance methods support discovery. `DriverManager` normally selects the driver.
pub use crate::core::Driver;
