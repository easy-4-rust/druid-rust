use super::BasicDataSourceMBean;

/// Apache DBCP 1 managed 管理契约。
///
/// 对应 Java: `org.apache.commons.dbcp.ManagedBasicDataSourceMBean`。
pub trait ManagedBasicDataSourceMBean: BasicDataSourceMBean {}

impl<T> ManagedBasicDataSourceMBean for T where T: BasicDataSourceMBean + ?Sized {}
