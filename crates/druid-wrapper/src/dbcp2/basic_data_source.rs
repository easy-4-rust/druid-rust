/// Apache DBCP 2 基础数据源兼容类型。
///
/// 对应 Java: `org.apache.commons.dbcp2.BasicDataSource`。原类没有新增状态或
/// 行为，直接复用 canonical Druid 数据源。
pub type BasicDataSource = druid_core::pool::DruidDataSource;
