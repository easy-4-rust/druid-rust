/// Apache DBCP 1 managed 数据源兼容类型。
///
/// 对应 Java: `org.apache.commons.dbcp.ManagedBasicDataSource`。原类只继承
/// `BasicDataSource`，故与 canonical Druid 数据源共享实现。
pub type ManagedBasicDataSource = druid::pool::DruidDataSource;
