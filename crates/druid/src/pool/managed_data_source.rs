/// 可运行期启停并暴露管理对象名的数据源。
///
/// 对应 Java: `com.alibaba.druid.pool.ManagedDataSource`。Java 的
/// `javax.management.ObjectName` 在 Rust 管理边界表示为已经校验的字符串，
/// 避免把 JMX 平台类型泄漏到连接池核心。
pub trait ManagedDataSource: Send + Sync {
    /// 返回数据源是否允许继续借出连接。
    fn is_enable(&self) -> bool;

    /// 设置数据源是否允许继续借出连接。
    ///
    /// 已经借出的连接不被强制中断；禁用只阻止新的获取，对应 Java
    /// `ManagedDataSource#setEnable` 的管理开关。
    fn set_enable(&self, value: bool);

    /// 返回管理对象名称。
    fn object_name(&self) -> Option<String>;

    /// 设置管理对象名称；`None` 对应 Java `null`。
    fn set_object_name(&self, object_name: Option<String>);
}
