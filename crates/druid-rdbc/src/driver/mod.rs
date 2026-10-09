//! 驱动、连接工厂与纯标准注册表。

mod connection_factory;
#[expect(
    clippy::module_inception,
    reason = "one standard object per file keeps mod.rs declaration-only"
)]
mod driver;
mod driver_action;
mod driver_extension_descriptor;
mod driver_registry;

pub use connection_factory::ConnectionFactory;
pub use driver::{ConnectionProperties, Driver, DriverPropertyInfo};
pub use driver_action::DriverAction;
pub use driver_extension_descriptor::DriverExtensionDescriptor;
pub use driver_registry::DriverRegistry;
