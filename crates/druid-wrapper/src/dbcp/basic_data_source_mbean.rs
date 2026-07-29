use druid::pool::ManagedDataSource;

/// Apache DBCP 1 管理契约。
///
/// 对应 Java: `org.apache.commons.dbcp.BasicDataSourceMBean`，其全部能力来自
/// `DruidDataSourceMBean`；Rust 映射到运行时启停与对象名管理契约。
pub trait BasicDataSourceMBean: ManagedDataSource {}

impl<T> BasicDataSourceMBean for T where T: ManagedDataSource + ?Sized {}
