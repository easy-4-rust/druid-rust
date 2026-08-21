/// Apache DBCP 1 基础数据源兼容类型。
///
/// 对应 Java: `org.apache.commons.dbcp.BasicDataSource`。Java 类只继承
/// `DruidDataSource` 并声明序列化 ID，没有额外行为。
pub type BasicDataSource = druid::pool::DruidDataSource;
