use druid::pool::ManagedDataSource;

/// Apache DBCP 2 管理契约。
///
/// 对应 Java: `org.apache.commons.dbcp2.BasicDataSourceMBean`。
pub trait BasicDataSourceMBean: ManagedDataSource {}

impl<T> BasicDataSourceMBean for T where T: ManagedDataSource + ?Sized {}
