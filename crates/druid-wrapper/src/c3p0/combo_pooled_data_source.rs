/// c3p0 `ComboPooledDataSource` 的 Druid 兼容实现。
///
/// 对应 Java: `com.mchange.v2.c3p0.ComboPooledDataSource`。Java 对象仅继承
/// `DruidDataSourceC3P0Adapter` 且没有新增状态或方法；Rust 因而保留同一
/// canonical 类型名，并直接复用唯一 native Druid 数据源，避免池中池。
pub type ComboPooledDataSource = druid_core::pool::DruidDataSource;
